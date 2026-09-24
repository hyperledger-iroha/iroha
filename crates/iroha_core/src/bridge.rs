//! Helpers for bridge finality proofs built from commit certificates.
use crate::{
    state::{State as CoreState, StateReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf, sha256};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader, SignedBlock,
        consensus_v2::finality::{V2FinalityArtifact, V2QuorumCertificateVerificationError},
        consensus_v2::{MAX_VALIDATORS_PER_HEIGHT, PROTOCOL_VERSION, SumeragiV2Status},
        execution_output::ExecutionOutputV1,
    },
    bridge::{
        BRIDGE_FINALITY_ATTESTATION_VERSION_V1, BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeCommitment,
        BridgeFinalityAttestationBodyV1, BridgeFinalityAttestationV1,
        BridgeFinalityAttestationValidationError, BridgeFinalityBundle, BridgeFinalityProof,
        SccpGovernedRouteV1, SccpLaneIdV1, SccpNetworkV1, SccpOutboundMessageKeyV1,
        SccpReplayAccumulatorIdV1, SccpReplayActorV1, SccpReplayBoundaryV1, SccpReplayDomainV1,
        SccpReplayForestV1, SccpReplayPrincipalV1, SccpReplayRecordV1, SccpRouteKeyV1,
        SccpSoraFinalityAnchorV1, sccp_sora_taira_chain_id_hash_v1,
    },
    isi::InstructionBox,
    transaction::{Executable, ExecutableBatchItem, TransactionEntrypoint, TransactionResult},
};
use iroha_model_base::name::Name;
use iroha_model_base::peer::PeerId;
use iroha_sccp::{
    SccpGroth16Bn254ProofRequestV1, SccpHubCommitmentV1, SccpPayloadV1, SccpReplayArchiveV1,
    TairaBridgeFinalityProofV1, TairaSccpMessageProofV1,
};
use mv::storage::StorageReadOnly;
use sha2::Digest as _;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::NonZeroUsize,
};
use thiserror::Error;
/// A Sumeragi-v2 finality artifact whose structure, roster PoPs, and CommitQC
/// cryptography have already been verified.
///
/// The wrapper is intentionally not decodable and exposes no mutable access.
/// Untrusted implementations of [`BridgeStateReadOnly`] must call [`Self::verify_for_header`]
/// to mint it. Kura-backed implementations use the private constructor only
/// after Kura's cache-backed verification boundary succeeds, and attach the
/// header authenticated by Kura's private durable finality record.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct VerifiedV2FinalityArtifact {
    artifact: V2FinalityArtifact,
    retained_header: BlockHeader,
}
/// Failure to derive an SCCP SORA anchor from authenticated Sumeragi-v2 finality.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SccpSoraFinalityAnchorBuildError {
    /// The authenticated finality belongs to another genesis lineage.
    #[error("SCCP finality anchor must use the exact SORA Taira network identity")]
    NetworkIdentity,
    /// SCCP does not admit election epoch zero.
    #[error("SCCP finality anchor epoch must be nonzero")]
    EpochZero,
    /// The checkpoint lies beyond its authenticated epoch window.
    #[error("SCCP finality anchor checkpoint exceeds its authenticated epoch end")]
    CheckpointAfterEpochEnd,
    /// Ordinary production finality must authenticate the exact parent decision.
    #[error("SCCP finality anchor requires an authenticated parent CommitQC")]
    MissingParentCommitQc,
    /// Snapshot-bootstrap finality is not an admissible SCCP trust anchor.
    #[error("SCCP finality anchor cannot use a snapshot bootstrap")]
    SnapshotBootstrap,
    /// The authenticated roster or aligned PoP inventory is not canonically hashable.
    #[error("SCCP finality anchor has an invalid authenticated roster")]
    InvalidAuthenticatedRoster,
    /// The derived anchor failed its closed final-V1 model validation.
    #[error("derived SCCP finality anchor is invalid")]
    InvalidDerivedAnchor,
}

const SCCP_SORA_ROSTER_SEMANTIC_DOMAIN_V1: &[u8] = b"iroha:sumeragi:v2:roster-semantic:final-v1";

impl VerifiedV2FinalityArtifact {
    /// Fully verify an untrusted artifact against its exact retained header.
    ///
    /// # Errors
    ///
    /// Returns the canonical v2 verification error when structural, PoP, or
    /// CommitQC cryptographic validation fails.
    pub fn verify_for_header(
        retained_header: BlockHeader,
        artifact: V2FinalityArtifact,
    ) -> Result<Self, V2QuorumCertificateVerificationError> {
        artifact.verify()?;
        artifact
            .validate_for_header(&retained_header)
            .map_err(V2QuorumCertificateVerificationError::InvalidArtifact)?;
        Ok(Self {
            artifact,
            retained_header,
        })
    }
    /// Borrow the verified artifact without allowing mutation.
    #[must_use]
    pub const fn artifact(&self) -> &V2FinalityArtifact {
        &self.artifact
    }
    /// Consume the wrapper and return the verified artifact.
    #[must_use]
    pub fn into_artifact(self) -> V2FinalityArtifact {
        self.artifact
    }
    /// Borrow Kura's authenticated retained header when one accompanied the artifact.
    #[must_use]
    pub const fn retained_header(&self) -> &BlockHeader {
        &self.retained_header
    }
    /// Derive the exact epoch-aware SCCP SORA anchor from verified finality.
    ///
    /// The election epoch, epoch end, ordered roster, and aligned proofs of
    /// possession come only from this unforgeable verified-finality capability.
    /// Genesis and snapshot-bootstrap artifacts are deliberately ineligible.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the artifact is not ordinary Taira finality,
    /// its SCCP epoch window is inadmissible, or its authenticated roster cannot
    /// be projected into the fixed 31-slot final-V1 commitment.
    pub fn sccp_sora_finality_anchor_v1(
        &self,
    ) -> Result<SccpSoraFinalityAnchorV1, SccpSoraFinalityAnchorBuildError> {
        let artifact = &self.artifact;
        let context = &artifact.height_context;
        if context.network_id != iroha_sccp::sccp_taira_finality_network_id_v1() {
            return Err(SccpSoraFinalityAnchorBuildError::NetworkIdentity);
        }
        if context.epoch == 0 {
            return Err(SccpSoraFinalityAnchorBuildError::EpochZero);
        }
        if artifact.height > context.epoch_end_height {
            return Err(SccpSoraFinalityAnchorBuildError::CheckpointAfterEpochEnd);
        }
        if context.parent_commit_qc.is_none() {
            return Err(SccpSoraFinalityAnchorBuildError::MissingParentCommitQc);
        }
        if context.snapshot_bootstrap.is_some() {
            return Err(SccpSoraFinalityAnchorBuildError::SnapshotBootstrap);
        }
        let roster_commitment =
            sccp_sora_roster_commitment_v1(&context.roster, &artifact.validator_set_pops)
                .ok_or(SccpSoraFinalityAnchorBuildError::InvalidAuthenticatedRoster)?;
        let anchor = SccpSoraFinalityAnchorV1 {
            version: 1,
            source_network: SccpNetworkV1::SoraTaira,
            protocol_version: PROTOCOL_VERSION,
            chain_id_hash: sccp_sora_taira_chain_id_hash_v1(),
            epoch: context.epoch,
            epoch_end_height: context.epoch_end_height,
            roster_commitment,
            checkpoint_height: artifact.height,
            checkpoint_block_hash: <[u8; 32]>::from(Hash::from(self.retained_header.hash())),
            checkpoint_context_id: <[u8; 32]>::from(Hash::from(artifact.context_id().0)),
            checkpoint_finality_artifact_hash: <[u8; 32]>::from(Hash::new(
                norito::codec::Encode::encode(artifact),
            )),
        };
        anchor
            .validate()
            .map_err(|_| SccpSoraFinalityAnchorBuildError::InvalidDerivedAnchor)?;
        Ok(anchor)
    }
    fn from_kura_verified(block_header: BlockHeader, artifact: V2FinalityArtifact) -> Self {
        Self {
            artifact,
            retained_header: block_header,
        }
    }
}

fn sccp_sora_roster_commitment_v1(
    roster: &[iroha_data_model::block::consensus_v2::ValidatorPower],
    validator_set_pops: &[Vec<u8>],
) -> Option<[u8; 32]> {
    if roster.is_empty()
        || roster.len() > MAX_VALIDATORS_PER_HEIGHT
        || roster.len() != validator_set_pops.len()
    {
        return None;
    }
    let validator_count = u32::try_from(roster.len()).ok()?;
    let mut preimage = Vec::with_capacity(
        SCCP_SORA_ROSTER_SEMANTIC_DOMAIN_V1.len()
            + 1
            + core::mem::size_of::<u32>()
            + MAX_VALIDATORS_PER_HEIGHT * 64,
    );
    preimage.extend_from_slice(SCCP_SORA_ROSTER_SEMANTIC_DOMAIN_V1);
    preimage.push(u8::try_from(PROTOCOL_VERSION).ok()?);
    preimage.extend_from_slice(&validator_count.to_le_bytes());
    for (entry, pop) in roster.iter().zip(validator_set_pops) {
        let (algorithm, public_key) = entry.validator.public_key().try_to_bytes().ok()?;
        if algorithm != Algorithm::BlsNormal || public_key.len() != 48 || pop.len() != 96 {
            return None;
        }
        preimage.extend_from_slice(&sha256(public_key));
        preimage.extend_from_slice(&sha256(pop));
    }
    preimage.resize(
        SCCP_SORA_ROSTER_SEMANTIC_DOMAIN_V1.len()
            + 1
            + core::mem::size_of::<u32>()
            + MAX_VALIDATORS_PER_HEIGHT * 64,
        0,
    );
    Some(*Hash::new(preimage).as_ref())
}
/// Narrow read-only surface used by bridge finality proof builders.
///
/// This keeps bridge-proof construction independent from full `StateView` snapshots.
pub trait BridgeStateReadOnly {
    /// Exact genesis-derived network identity bound to the state snapshot.
    fn bridge_network_id(&self) -> &NetworkId;
    /// Load an exact durable Sumeragi-v2 finality artifact whose structure, roster PoPs, and
    /// CommitQC cryptography have already been verified by the storage boundary.
    fn bridge_verified_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String>;
    /// Load verified finality and Kura's immutable SCCP archive in one bounded pass.
    ///
    /// Implementations must authenticate the exact retained header and deterministic
    /// commitment-index order without falling back to block bodies or mutable WSV payloads.
    fn bridge_verified_v2_finality_with_sccp_archive(
        &self,
        height: u64,
    ) -> Result<
        Option<(
            VerifiedV2FinalityArtifact,
            Vec<ValidatedSccpOutboundMessageProjectionV1>,
        )>,
        String,
    >;
}
impl<T: StateReadOnly> BridgeStateReadOnly for T {
    fn bridge_network_id(&self) -> &NetworkId {
        self.network_id()
    }
    fn bridge_verified_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String> {
        self.kura()
            .v2_finality_artifact_with_header(height)
            .map(|record| {
                record.map(|(header, artifact)| {
                    VerifiedV2FinalityArtifact::from_kura_verified(header, artifact)
                })
            })
            .map_err(|error| error.to_string())
    }
    fn bridge_verified_v2_finality_with_sccp_archive(
        &self,
        height: u64,
    ) -> Result<
        Option<(
            VerifiedV2FinalityArtifact,
            Vec<ValidatedSccpOutboundMessageProjectionV1>,
        )>,
        String,
    > {
        self.kura()
            .v2_finality_artifact_with_archive(height)
            .map(|record| {
                record.map(|(header, artifact, archive)| {
                    (
                        VerifiedV2FinalityArtifact::from_kura_verified(header, artifact),
                        archive,
                    )
                })
            })
            .map_err(|error| error.to_string())
    }
}
impl BridgeStateReadOnly for CoreState {
    fn bridge_network_id(&self) -> &NetworkId {
        self.network_id_ref()
    }
    fn bridge_verified_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String> {
        self.kura()
            .v2_finality_artifact_with_header(height)
            .map(|record| {
                record.map(|(header, artifact)| {
                    VerifiedV2FinalityArtifact::from_kura_verified(header, artifact)
                })
            })
            .map_err(|error| error.to_string())
    }
    fn bridge_verified_v2_finality_with_sccp_archive(
        &self,
        height: u64,
    ) -> Result<
        Option<(
            VerifiedV2FinalityArtifact,
            Vec<ValidatedSccpOutboundMessageProjectionV1>,
        )>,
        String,
    > {
        self.kura()
            .v2_finality_artifact_with_archive(height)
            .map(|record| {
                record.map(|(header, artifact, archive)| {
                    (
                        VerifiedV2FinalityArtifact::from_kura_verified(header, artifact),
                        archive,
                    )
                })
            })
            .map_err(|error| error.to_string())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
/// Decoded SCCP message plus its location in a transaction stream.
pub struct RecordedSccpMessage {
    /// Zero-based index of the transaction that emitted the SCCP message.
    pub tx_index: usize,
    /// Zero-based index of the instruction within the transaction executable.
    pub instruction_index: usize,
    /// Exact governed lane and destination binding supplied by the instruction.
    pub context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    /// Canonically decoded SCCP payload recorded by the instruction.
    pub payload: SccpPayloadV1,
    /// Commitment derived from the decoded SCCP payload.
    pub commitment: SccpHubCommitmentV1,
}
/// Canonically validated outbound SCCP record data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedRecordedSccpMessage {
    /// Exact structurally validated outbound context.
    pub context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    /// Canonically decoded SCCP payload recorded by the instruction.
    pub payload: SccpPayloadV1,
    /// Exact lane-bound outbound replay key derived from the context and payload.
    pub key: SccpOutboundMessageKeyV1,
    /// Canonical SCCP hub commitment for the payload.
    pub commitment: SccpHubCommitmentV1,
}
/// Location-free, fully validated projection of one finalized SCCP outbox message.
///
/// The projection is safe for read APIs to render directly: Core has verified exact canonical
/// payload framing and semantics, the lane-bound message identifier, all structural context
/// roles, and the payload commitment against the supplied durable record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSccpOutboundMessageProjectionV1 {
    /// Zero-based position authenticated by the block's SCCP commitment root.
    pub commitment_index: u32,
    /// Exact outbound lane and governed binding context retained by the record.
    pub context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    /// Canonically decoded SCCP V1 application payload.
    pub payload: SccpPayloadV1,
    /// Recomputed lane-, context-, message-, and payload-bound hub commitment.
    pub commitment: SccpHubCommitmentV1,
}
impl ValidatedRecordedSccpMessage {
    /// Return the exact governed external-to-SORA route key for this outbound record.
    ///
    /// Although the message lane points from SORA to the destination, registry
    /// keys use the reciprocal external-to-SORA lane. Keeping this derivation
    /// beside canonical payload validation prevents scheduler and execution
    /// admission from disagreeing about the route-scoped state they read.
    pub(crate) fn governed_route_key(&self) -> Option<SccpRouteKeyV1> {
        let SccpPayloadV1::Transfer(transfer) = &self.payload;
        replay_route_key(
            SccpLaneIdV1 {
                source: self.context.lane.target,
                target: self.context.lane.source,
            },
            transfer,
        )
        .ok()
    }

    /// Build the payload-bearing pending outbox record from this canonical validation result.
    pub(crate) fn outbound_record(
        &self,
        recorded_at_height: u64,
        commitment_index: u32,
    ) -> Option<iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1> {
        let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&self.payload).ok()?;
        let record = iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1 {
            destination_binding_hash: self.context.destination_binding_hash,
            route_configuration_hash: self.context.route_configuration_hash,
            payload_hash: self.commitment.payload_hash,
            payload_bytes,
            recorded_at_height,
            commitment_index,
        };
        validate_sccp_outbound_message_record_internal(&self.key, &record)
            .is_ok()
            .then_some(record)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedSccpMessageCandidate {
    instruction_index: usize,
    validated: ValidatedRecordedSccpMessage,
}
/// Failure while validating a recorded outbound SCCP message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecordedSccpMessageValidationError {
    /// The supplied context is not an exact SORA-to-external lane with a nonzero binding.
    InvalidContext,
    /// Payload bytes are not exact canonical SCCP variant framing.
    InvalidPayload,
    /// Outbound records may only originate from SORA.
    NonSoraSource {
        /// Source domain found in the payload.
        source_domain: u32,
    },
    /// The payload destination domain does not match the context's exact target profile.
    TargetProfileMismatch {
        /// Exact target profile declared by the context.
        target: iroha_data_model::bridge::SccpNetworkV1,
        /// SCCP destination domain encoded by the payload.
        payload_target_domain: u32,
    },
    /// The destination binding, lane-bound message id, and payload hash are not distinct.
    HashRoleCollision,
    /// Payload route fields are not bound to the deterministic outbound route.
    RouteBinding {
        /// Route binding validation error.
        error: SccpOutboundRouteValidationError,
    },
}
pub(crate) fn decode_recorded_sccp_payload_bytes(payload_bytes: &[u8]) -> Option<SccpPayloadV1> {
    let payload = iroha_sccp::decode_canonical_sccp_payload_bytes(payload_bytes)?;
    if iroha_sccp::canonical_sccp_payload_bytes(&payload)
        .ok()?
        .as_slice()
        != payload_bytes
    {
        return None;
    }
    Some(payload)
}
#[cfg(test)]
fn test_sccp_target_network_for_domain(
    target_domain: u32,
) -> iroha_data_model::bridge::SccpNetworkV1 {
    use iroha_data_model::bridge::SccpNetworkV1;
    match target_domain {
        iroha_sccp::SCCP_DOMAIN_ETH => SccpNetworkV1::EthereumMainnet,
        iroha_sccp::SCCP_DOMAIN_BSC => SccpNetworkV1::BscMainnet,
        iroha_sccp::SCCP_DOMAIN_TON => SccpNetworkV1::TonMainnet,
        iroha_sccp::SCCP_DOMAIN_TRON => SccpNetworkV1::TronMainnet,
        domain => panic!("test SCCP payload names unsupported target domain {domain}"),
    }
}
#[cfg(test)]
pub(crate) fn test_sccp_outbound_context_for_payload_bytes(
    payload_bytes: &[u8],
) -> iroha_data_model::bridge::SccpOutboundMessageContextV1 {
    use iroha_data_model::bridge::{SccpLaneIdV1, SccpNetworkV1};
    let target_domain = decode_recorded_sccp_payload_bytes(payload_bytes)
        .map(|payload| iroha_sccp::sccp_message_target_domain(&payload))
        .filter(|domain| *domain != iroha_sccp::SCCP_DOMAIN_SORA)
        .unwrap_or(iroha_sccp::SCCP_DOMAIN_ETH);
    let target = test_sccp_target_network_for_domain(target_domain);
    let (destination_binding_hash, route_configuration_hash) = if matches!(
        target_domain,
        iroha_sccp::SCCP_DOMAIN_ETH | iroha_sccp::SCCP_DOMAIN_BSC
    ) {
        let route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
            target,
            iroha_data_model::bridge::SccpRouteActivationV1::Staged,
        );
        (
            route
                .destination_binding_hash()
                .expect("exact test EVM destination binding"),
            route
                .route_configuration_hash()
                .expect("exact test EVM route configuration"),
        )
    } else {
        ([0x36; 32], [0x37; 32])
    };
    iroha_data_model::bridge::SccpOutboundMessageContextV1::new(
        SccpLaneIdV1 {
            source: SccpNetworkV1::SoraTaira,
            target,
        },
        destination_binding_hash,
        route_configuration_hash,
    )
    .expect("test SCCP outbound context must be valid")
}
#[cfg(test)]
pub(crate) fn test_record_sccp_message(
    payload_bytes: Vec<u8>,
) -> iroha_data_model::isi::bridge::RecordSccpMessage {
    let context = test_sccp_outbound_context_for_payload_bytes(&payload_bytes);
    iroha_data_model::isi::bridge::RecordSccpMessage::new(
        context,
        payload_bytes,
        iroha_data_model::bridge::SccpSparseMerkleWitnessV1::empty_shard(),
    )
}
#[cfg(test)]
pub(crate) fn test_sccp_outbound_message_key(payload: &SccpPayloadV1) -> SccpOutboundMessageKeyV1 {
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(payload)
        .expect("valid SCCP outbound-key fixture payload encodes");
    let context = test_sccp_outbound_context_for_payload_bytes(&payload_bytes);
    sccp_outbound_message_key(context.lane, payload).expect("test SCCP outbound key must be valid")
}
#[cfg(test)]
pub(crate) fn test_sccp_hub_commitment(payload: &SccpPayloadV1) -> SccpHubCommitmentV1 {
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(payload)
        .expect("valid SCCP commitment fixture payload encodes");
    let context = test_sccp_outbound_context_for_payload_bytes(&payload_bytes);
    iroha_sccp::hub_commitment_from_sccp_payload(context, payload)
        .expect("test SCCP hub commitment must be valid")
}
fn validate_recorded_sccp_payload(
    context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    payload: SccpPayloadV1,
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    if !context.is_well_formed() {
        return Err(RecordedSccpMessageValidationError::InvalidContext);
    }
    let source_domain = iroha_sccp::sccp_message_source_domain(&payload);
    if source_domain != iroha_sccp::SCCP_DOMAIN_SORA {
        return Err(RecordedSccpMessageValidationError::NonSoraSource { source_domain });
    }
    let payload_target_domain = iroha_sccp::sccp_message_target_domain(&payload);
    if payload_target_domain != context.lane.target.domain_id() {
        return Err(RecordedSccpMessageValidationError::TargetProfileMismatch {
            target: context.lane.target,
            payload_target_domain,
        });
    }
    validate_sora_outbound_sccp_payload_route(&payload)
        .map_err(|error| RecordedSccpMessageValidationError::RouteBinding { error })?;
    if !iroha_sccp::verify_sccp_payload_structure(&payload) {
        return Err(RecordedSccpMessageValidationError::InvalidPayload);
    }
    let key = sccp_outbound_message_key(context.lane, &payload)
        .ok_or(RecordedSccpMessageValidationError::InvalidContext)?;
    let commitment = iroha_sccp::hub_commitment_from_sccp_payload(context, &payload)
        .ok_or(RecordedSccpMessageValidationError::InvalidContext)?;
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&payload)
        .map_err(|_| RecordedSccpMessageValidationError::InvalidPayload)?;
    let durable = iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1 {
        destination_binding_hash: context.destination_binding_hash,
        route_configuration_hash: context.route_configuration_hash,
        payload_hash: commitment.payload_hash,
        payload_bytes,
        recorded_at_height: 1,
        commitment_index: 0,
    };
    if !durable.is_well_formed_for_key(&key) {
        return Err(RecordedSccpMessageValidationError::HashRoleCollision);
    }
    Ok(ValidatedRecordedSccpMessage {
        context,
        key,
        commitment,
        payload,
    })
}
pub(crate) fn validate_recorded_sccp_message_payload_bytes(
    context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    payload_bytes: &[u8],
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    if payload_bytes.is_empty()
        || payload_bytes.len()
            > iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1
    {
        return Err(RecordedSccpMessageValidationError::InvalidPayload);
    }
    let payload = decode_recorded_sccp_payload_bytes(payload_bytes)
        .ok_or(RecordedSccpMessageValidationError::InvalidPayload)?;
    validate_recorded_sccp_payload(context, payload)
}
/// Fully revalidate one payload-bearing pending SCCP outbox record against its replay key.
///
/// This is intentionally stronger than the data-model structural predicate: it decodes and
/// re-encodes the retained payload, verifies SCCP V1 semantics, recomputes the lane-bound message
/// identifier and payload commitment, and binds both governed context hashes to the record.
fn validate_sccp_outbound_message_record_internal(
    key: &SccpOutboundMessageKeyV1,
    record: &iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1,
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    if !record.is_well_formed_for_key(key) {
        return Err(RecordedSccpMessageValidationError::InvalidContext);
    }
    let context = iroha_data_model::bridge::SccpOutboundMessageContextV1 {
        lane: key.lane,
        destination_binding_hash: record.destination_binding_hash,
        route_configuration_hash: record.route_configuration_hash,
    };
    let validated = validate_recorded_sccp_message_payload_bytes(context, &record.payload_bytes)?;
    if validated.key != *key || validated.commitment.payload_hash != record.payload_hash {
        return Err(RecordedSccpMessageValidationError::InvalidPayload);
    }
    Ok(validated)
}
/// Validate and project one payload-bearing pending SCCP outbox record.
///
/// Returns `None` unless the retained bytes are bounded, exact canonical SCCP V1 framing; decode
/// to a structurally and semantically valid SORA-origin payload for the supplied lane; recompute
/// the supplied lane-bound message identifier; and match the record's payload hash and governed
/// context roles. Registry hydration separately proves those context hashes name one retained
/// governed route before the state becomes observable.
#[must_use]
pub fn validate_sccp_outbound_message_record_v1(
    key: &SccpOutboundMessageKeyV1,
    record: &iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1,
) -> Option<ValidatedSccpOutboundMessageProjectionV1> {
    let validated = validate_sccp_outbound_message_record_internal(key, record).ok()?;
    Some(ValidatedSccpOutboundMessageProjectionV1 {
        commitment_index: record.commitment_index,
        context: validated.context,
        payload: validated.payload,
        commitment: validated.commitment,
    })
}
/// Return the next dense commitment index for one block height.
///
/// `None` means the height already contains the exact first-release maximum. The range includes
/// writes staged by the current state transaction, so dropping a failed transaction also releases
/// every index it tentatively allocated.
pub(crate) fn next_sccp_outbound_commitment_index(
    ordered: &impl StorageReadOnly<iroha_data_model::bridge::SccpOutboundMessageIndexKeyV1, ()>,
    height: u64,
) -> Result<Option<u32>, String> {
    if height == 0 {
        return Err("SCCP outbound commitment height must be nonzero".to_owned());
    }
    let max = iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1;
    let start =
        iroha_data_model::bridge::SccpOutboundMessageIndexKeyV1::range_start_at_or_before(height);
    let mut expected = 0_u32;
    for (index, ()) in ordered.range(start..) {
        if index.recorded_at_height != height {
            break;
        }
        if expected >= max {
            return Err(format!(
                "SCCP outbound index at height {height} exceeds the {max}-message block bound"
            ));
        }
        if !index.is_well_formed() || index.commitment_index != expected {
            return Err(format!(
                "SCCP outbound index at height {height} is not dense: expected {expected}, found {}",
                index.commitment_index
            ));
        }
        expected += 1;
    }
    Ok((expected < max).then_some(expected))
}
fn validate_recorded_sccp_message_payload_bytes_for_block_collection(
    context: iroha_data_model::bridge::SccpOutboundMessageContextV1,
    payload_bytes: &[u8],
) -> Result<ValidatedRecordedSccpMessage, RecordedSccpMessageValidationError> {
    validate_recorded_sccp_message_payload_bytes(context, payload_bytes)
}
pub(crate) fn sccp_outbound_message_key(
    lane: iroha_data_model::bridge::SccpLaneIdV1,
    payload: &SccpPayloadV1,
) -> Option<SccpOutboundMessageKeyV1> {
    SccpOutboundMessageKeyV1::new(lane, iroha_sccp::sccp_message_id(lane, payload)?)
}
fn recorded_sccp_message_instruction(
    instruction: &InstructionBox,
) -> Option<&iroha_data_model::isi::bridge::RecordSccpMessage> {
    instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::bridge::RecordSccpMessage>()
}
fn validate_recorded_sccp_message_instruction(
    instruction: &InstructionBox,
) -> Result<Option<ValidatedRecordedSccpMessage>, RecordedSccpMessageValidationError> {
    let Some(record) = recorded_sccp_message_instruction(instruction) else {
        return Ok(None);
    };
    validate_recorded_sccp_message_payload_bytes(record.context, &record.payload_bytes).map(Some)
}
fn signed_transaction_from_sccp_entrypoint(
    entrypoint: &TransactionEntrypoint,
) -> Option<&iroha_data_model::transaction::SignedTransaction> {
    match entrypoint {
        TransactionEntrypoint::External(transaction) => Some(transaction),
        TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
        TransactionEntrypoint::SealedCommitment(_) => None,
    }
}
fn entrypoint_has_successful_or_pending_result(
    block: &SignedBlock,
    entrypoint_index: usize,
) -> bool {
    if !block.has_results() {
        return true;
    }
    u32::try_from(entrypoint_index)
        .ok()
        .and_then(|index| block.network_output_at(index))
        .is_some_and(|(_, row)| row.result.as_ref().is_ok())
}
/// Invalid route binding for a SORA-origin outbound SCCP payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SccpOutboundRouteValidationError {
    /// Route id is not encoded as SCCP `canonical_text`.
    NonTextRouteId,
    /// Asset id is not encoded as SCCP `canonical_text`.
    NonTextAssetId,
    /// Route id bytes are not valid UTF-8.
    InvalidRouteIdUtf8,
    /// Text route id is empty.
    EmptyRouteId,
    /// Asset id bytes are not valid UTF-8.
    InvalidAssetIdUtf8,
    /// Text asset id has no route-local asset key.
    EmptyAssetKey,
    /// Text asset id route-local key is not an Iroha `Name`.
    InvalidAssetKey,
    /// Text asset id contains `#` without a scope suffix.
    EmptyAssetScope,
    /// Text asset id contains more than one `#` scope separator.
    AmbiguousAssetScope,
    /// Text asset id uses a scope suffix instead of the canonical route-local key.
    AssetScopeAlias {
        /// Route-local asset key extracted from the scoped spelling.
        asset_key: String,
        /// Scope suffix found in the payload.
        scope: String,
    },
    /// Asset home domain is not SORA in the first-release lock/release model.
    InvalidAssetHomeDomain {
        /// Asset home domain in the payload.
        asset_home_domain: u32,
        /// Destination domain in the payload.
        dest_domain: u32,
    },
}
impl SccpOutboundRouteValidationError {
    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::NonTextRouteId => "RecordSccpMessage payload route_id is not canonical_text",
            Self::NonTextAssetId => "RecordSccpMessage payload asset_id is not canonical_text",
            Self::InvalidRouteIdUtf8 => "RecordSccpMessage payload route_id is invalid UTF-8",
            Self::EmptyRouteId => "RecordSccpMessage payload route_id is empty",
            Self::InvalidAssetIdUtf8 => "RecordSccpMessage payload asset_id is invalid UTF-8",
            Self::EmptyAssetKey => "RecordSccpMessage payload asset key is empty",
            Self::InvalidAssetKey => "RecordSccpMessage payload asset key is not a valid Name",
            Self::EmptyAssetScope => "RecordSccpMessage payload asset scope is empty",
            Self::AmbiguousAssetScope => {
                "RecordSccpMessage payload asset_id has multiple scope separators"
            }
            Self::AssetScopeAlias { .. } => {
                "RecordSccpMessage payload asset_id must be the canonical route-local asset key"
            }
            Self::InvalidAssetHomeDomain { .. } => {
                "RecordSccpMessage payload asset home domain is not SORA"
            }
        }
    }
}
impl fmt::Display for SccpOutboundRouteValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonTextRouteId => write!(f, "route_id must use canonical_text codec"),
            Self::NonTextAssetId => write!(f, "asset_id must use canonical_text codec"),
            Self::InvalidRouteIdUtf8 => write!(f, "route_id must be valid UTF-8"),
            Self::EmptyRouteId => write!(f, "route_id must not be empty"),
            Self::InvalidAssetIdUtf8 => write!(f, "asset_id must be valid UTF-8"),
            Self::EmptyAssetKey => write!(f, "asset key must not be empty"),
            Self::InvalidAssetKey => write!(f, "asset key must be a valid Iroha Name"),
            Self::EmptyAssetScope => write!(f, "asset scope must not be empty after `#`"),
            Self::AmbiguousAssetScope => {
                write!(f, "asset_id must contain at most one `#` scope separator")
            }
            Self::AssetScopeAlias { asset_key, scope } => write!(
                f,
                "asset_id must be canonical route-local key `{asset_key}`, not scoped alias `{asset_key}#{scope}`"
            ),
            Self::InvalidAssetHomeDomain {
                asset_home_domain,
                dest_domain: _,
            } => write!(f, "asset home domain {asset_home_domain} must be SORA"),
        }
    }
}
fn sccp_text_route_field<'a>(
    codec: u8,
    bytes: &'a [u8],
    non_text: SccpOutboundRouteValidationError,
    invalid_utf8: SccpOutboundRouteValidationError,
) -> Result<&'a str, SccpOutboundRouteValidationError> {
    if codec != iroha_sccp::SCCP_CODEC_CANONICAL_TEXT {
        return Err(non_text);
    }
    core::str::from_utf8(bytes).map_err(|_| invalid_utf8)
}
fn sccp_route_asset_key(asset_id: &str) -> Result<&str, SccpOutboundRouteValidationError> {
    let mut parts = asset_id.split('#');
    let asset_key = parts.next().unwrap_or_default();
    if asset_key.is_empty() {
        return Err(SccpOutboundRouteValidationError::EmptyAssetKey);
    }
    if asset_key.parse::<Name>().is_err() {
        return Err(SccpOutboundRouteValidationError::InvalidAssetKey);
    }
    if let Some(scope) = parts.next() {
        if scope.is_empty() {
            return Err(SccpOutboundRouteValidationError::EmptyAssetScope);
        }
        if parts.next().is_some() {
            return Err(SccpOutboundRouteValidationError::AmbiguousAssetScope);
        }
        return Err(SccpOutboundRouteValidationError::AssetScopeAlias {
            asset_key: asset_key.to_owned(),
            scope: scope.to_owned(),
        });
    }
    Ok(asset_key)
}
fn validate_sora_outbound_transfer_route(
    transfer: &iroha_sccp::TransferPayloadV1,
) -> Result<(), SccpOutboundRouteValidationError> {
    let route_id = sccp_text_route_field(
        transfer.route_id_codec,
        transfer.route_id.as_slice(),
        SccpOutboundRouteValidationError::NonTextRouteId,
        SccpOutboundRouteValidationError::InvalidRouteIdUtf8,
    )?;
    let asset_id = sccp_text_route_field(
        transfer.asset_id_codec,
        transfer.asset_id.as_slice(),
        SccpOutboundRouteValidationError::NonTextAssetId,
        SccpOutboundRouteValidationError::InvalidAssetIdUtf8,
    )?;
    if route_id.is_empty() {
        return Err(SccpOutboundRouteValidationError::EmptyRouteId);
    }
    sccp_route_asset_key(asset_id)?;
    if transfer.asset_home_domain != iroha_sccp::SCCP_DOMAIN_SORA {
        return Err(SccpOutboundRouteValidationError::InvalidAssetHomeDomain {
            asset_home_domain: transfer.asset_home_domain,
            dest_domain: transfer.dest_domain,
        });
    }
    Ok(())
}
/// Validate deterministic route binding for SORA-origin outbound SCCP records.
pub(crate) fn validate_sora_outbound_sccp_payload_route(
    payload: &SccpPayloadV1,
) -> Result<(), SccpOutboundRouteValidationError> {
    let SccpPayloadV1::Transfer(transfer) = payload;
    validate_sora_outbound_transfer_route(transfer)
}
fn collect_sccp_messages_from_executable<F>(
    tx_index: usize,
    executable: &Executable,
    seen: &mut BTreeSet<SccpOutboundMessageKeyV1>,
    is_already_recorded: &F,
    deduplicate: bool,
    out: &mut Vec<RecordedSccpMessage>,
) where
    F: Fn(&SccpOutboundMessageKeyV1) -> bool,
{
    let mut push_instruction = |instruction_index: usize, instruction: &InstructionBox| {
        let Ok(Some(validated)) = validate_recorded_sccp_message_instruction(instruction) else {
            return;
        };
        let key = validated.key.clone();
        if is_already_recorded(&key) {
            return;
        }
        if deduplicate {
            if seen.contains(&key) {
                return;
            }
            seen.insert(key);
        }
        out.push(RecordedSccpMessage {
            tx_index,
            instruction_index,
            context: validated.context,
            commitment: validated.commitment,
            payload: validated.payload,
        });
    };
    match executable {
        Executable::Instructions(instructions) => {
            for (instruction_index, instruction) in instructions.iter().enumerate() {
                push_instruction(instruction_index, instruction);
            }
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {}
        Executable::Batch(items) => {
            for (item_index, item) in items.iter().enumerate() {
                if let ExecutableBatchItem::Instruction(instruction) = item {
                    push_instruction(item_index, instruction);
                }
            }
        }
        Executable::IvmProved(proved) => {
            for (instruction_index, instruction) in proved.overlay.iter().enumerate() {
                push_instruction(instruction_index, instruction);
            }
        }
    }
}
fn sccp_message_candidates_from_executable(
    executable: &Executable,
) -> Vec<RecordedSccpMessageCandidate> {
    if let Executable::Batch(items) = executable {
        return items
            .iter()
            .enumerate()
            .filter_map(|(instruction_index, item)| {
                let ExecutableBatchItem::Instruction(instruction) = item else {
                    return None;
                };
                let record = recorded_sccp_message_instruction(instruction)?;
                let Ok(validated) =
                    validate_recorded_sccp_message_payload_bytes_for_block_collection(
                        record.context,
                        &record.payload_bytes,
                    )
                else {
                    return None;
                };
                Some(RecordedSccpMessageCandidate {
                    instruction_index,
                    validated,
                })
            })
            .collect();
    }
    let instructions = match executable {
        Executable::Instructions(instructions) => instructions.as_ref(),
        Executable::IvmProved(proved) => proved.overlay.as_ref(),
        Executable::ContractCall(_) | Executable::Ivm(_) => return Vec::new(),
        Executable::Batch(_) => unreachable!("batch handled above"),
    };
    instructions
        .iter()
        .enumerate()
        .filter_map(|(instruction_index, instruction)| {
            let record = recorded_sccp_message_instruction(instruction)?;
            let Ok(validated) = validate_recorded_sccp_message_payload_bytes_for_block_collection(
                record.context,
                &record.payload_bytes,
            ) else {
                return None;
            };
            Some(RecordedSccpMessageCandidate {
                instruction_index,
                validated,
            })
        })
        .collect()
}
/// Extract all SCCP message records from accepted signed entrypoints.
pub fn collect_sccp_messages_from_accepted_transactions(
    transactions: &[AcceptedTransaction<'_>],
) -> Vec<RecordedSccpMessage> {
    collect_new_sccp_messages_from_accepted_transactions(transactions, |_| false)
}
/// Extract newly recordable SCCP message records from accepted signed entrypoints.
///
/// Existing outbox keys are excluded so proposal headers do not commit messages
/// that execution will reject as outbound replays.
pub fn collect_new_sccp_messages_from_accepted_transactions<F>(
    transactions: &[AcceptedTransaction<'_>],
    is_already_recorded: F,
) -> Vec<RecordedSccpMessage>
where
    F: Fn(&SccpOutboundMessageKeyV1) -> bool,
{
    collect_new_sccp_messages_from_accepted_transactions_where(
        transactions,
        |_| true,
        is_already_recorded,
    )
}
/// Extract newly recordable SCCP message records from selected accepted signed entrypoints.
///
/// The transaction-index filter preserves canonical block entrypoint indices in
/// the returned messages while letting proposal assembly exclude transactions
/// whose refreshed routing context cannot execute outbound SCCP records.
pub(crate) fn collect_new_sccp_messages_from_accepted_transactions_where<F, G>(
    transactions: &[AcceptedTransaction<'_>],
    include_transaction_index: F,
    is_already_recorded: G,
) -> Vec<RecordedSccpMessage>
where
    F: Fn(usize) -> bool,
    G: Fn(&SccpOutboundMessageKeyV1) -> bool,
{
    let mut messages = Vec::new();
    let mut seen = BTreeSet::new();
    for (tx_index, transaction) in transactions.iter().enumerate() {
        if !include_transaction_index(tx_index) {
            continue;
        }
        if let Some(signed) = signed_transaction_from_sccp_entrypoint(transaction.entrypoint()) {
            collect_sccp_messages_from_executable(
                tx_index,
                signed.instructions(),
                &mut seen,
                &is_already_recorded,
                true,
                &mut messages,
            );
        }
    }
    messages
}
fn collect_sccp_messages_from_signed_block_with_deduplication(
    block: &SignedBlock,
    deduplicate: bool,
) -> Vec<RecordedSccpMessage> {
    // Discovery is not acceptance: malformed attached outputs produce no candidates.
    // The committed validator below reports the exact structural refusal.
    if validate_sccp_execution_projection(block).is_err() {
        return Vec::new();
    }
    let mut messages = Vec::new();
    let mut seen = BTreeSet::new();
    for (entrypoint_index, entrypoint) in block.network_entrypoints().enumerate() {
        let Some(transaction) = signed_transaction_from_sccp_entrypoint(entrypoint) else {
            continue;
        };
        if !entrypoint_has_successful_or_pending_result(block, entrypoint_index) {
            continue;
        }
        let candidates = sccp_message_candidates_from_executable(transaction.instructions());
        for candidate in candidates {
            let key = candidate.validated.key.clone();
            if deduplicate {
                if seen.contains(&key) {
                    continue;
                }
                seen.insert(key);
            }
            messages.push(RecordedSccpMessage {
                tx_index: entrypoint_index,
                instruction_index: candidate.instruction_index,
                context: candidate.validated.context,
                commitment: candidate.validated.commitment,
                payload: candidate.validated.payload,
            });
        }
    }
    messages
}
/// Malformed committed SCCP record instruction found in a successful or pending entrypoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SccpRecordInstructionValidationError {
    /// `RecordSccpMessage` payload bytes could not be decoded as a valid SCCP payload.
    InvalidPayload {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
    },
    /// `RecordSccpMessage` context is malformed or does not describe an outbound lane.
    InvalidContext {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
    },
    /// `RecordSccpMessage` payload decoded, but its source domain is not SORA.
    NonSoraSource {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
        /// Source domain encoded in the SCCP payload.
        source_domain: u32,
    },
    /// Payload destination domain does not match the exact target profile.
    TargetProfileMismatch {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
        /// Exact target profile declared by the context.
        target: iroha_data_model::bridge::SccpNetworkV1,
        /// SCCP destination domain encoded by the payload.
        payload_target_domain: u32,
    },
    /// Context binding, message id, and payload hash collide across semantic roles.
    HashRoleCollision {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
    },
    /// `RecordSccpMessage` payload decoded, but outbound route binding is invalid.
    RouteBinding {
        /// External entrypoint index in the block payload.
        tx_index: usize,
        /// Instruction index inside the IVM overlay.
        instruction_index: usize,
        /// Route-binding validation error.
        error: SccpOutboundRouteValidationError,
    },
}
impl SccpRecordInstructionValidationError {
    pub(crate) fn tx_index(&self) -> usize {
        match self {
            Self::InvalidPayload { tx_index, .. }
            | Self::InvalidContext { tx_index, .. }
            | Self::NonSoraSource { tx_index, .. }
            | Self::TargetProfileMismatch { tx_index, .. }
            | Self::HashRoleCollision { tx_index, .. }
            | Self::RouteBinding { tx_index, .. } => *tx_index,
        }
    }
    pub(crate) fn instruction_index(&self) -> usize {
        match self {
            Self::InvalidPayload {
                instruction_index, ..
            }
            | Self::InvalidContext {
                instruction_index, ..
            }
            | Self::NonSoraSource {
                instruction_index, ..
            }
            | Self::TargetProfileMismatch {
                instruction_index, ..
            }
            | Self::HashRoleCollision {
                instruction_index, ..
            }
            | Self::RouteBinding {
                instruction_index, ..
            } => *instruction_index,
        }
    }
    pub(crate) fn reason(&self) -> &'static str {
        match self {
            Self::InvalidPayload { .. } => "RecordSccpMessage payload is invalid",
            Self::InvalidContext { .. } => "RecordSccpMessage context is invalid",
            Self::NonSoraSource { .. } => "RecordSccpMessage payload source domain is not SORA",
            Self::TargetProfileMismatch { .. } => {
                "RecordSccpMessage payload destination domain does not match its exact target profile"
            }
            Self::HashRoleCollision { .. } => {
                "RecordSccpMessage destination binding, message id, and payload hash must be distinct"
            }
            Self::RouteBinding { error, .. } => error.reason(),
        }
    }
}
fn invalid_sccp_record_instruction_in_executable(
    tx_index: usize,
    executable: &Executable,
) -> Option<SccpRecordInstructionValidationError> {
    let validate_one = |instruction_index: usize, instruction: &InstructionBox| {
        let record = recorded_sccp_message_instruction(instruction)?;
        match validate_recorded_sccp_message_payload_bytes(record.context, &record.payload_bytes) {
            Ok(_) => None,
            Err(RecordedSccpMessageValidationError::InvalidPayload) => {
                Some(SccpRecordInstructionValidationError::InvalidPayload {
                    tx_index,
                    instruction_index,
                })
            }
            Err(RecordedSccpMessageValidationError::InvalidContext) => {
                Some(SccpRecordInstructionValidationError::InvalidContext {
                    tx_index,
                    instruction_index,
                })
            }
            Err(RecordedSccpMessageValidationError::NonSoraSource { source_domain }) => {
                Some(SccpRecordInstructionValidationError::NonSoraSource {
                    tx_index,
                    instruction_index,
                    source_domain,
                })
            }
            Err(RecordedSccpMessageValidationError::TargetProfileMismatch {
                target,
                payload_target_domain,
            }) => Some(
                SccpRecordInstructionValidationError::TargetProfileMismatch {
                    tx_index,
                    instruction_index,
                    target,
                    payload_target_domain,
                },
            ),
            Err(RecordedSccpMessageValidationError::HashRoleCollision) => {
                Some(SccpRecordInstructionValidationError::HashRoleCollision {
                    tx_index,
                    instruction_index,
                })
            }
            Err(RecordedSccpMessageValidationError::RouteBinding { error }) => {
                Some(SccpRecordInstructionValidationError::RouteBinding {
                    tx_index,
                    instruction_index,
                    error,
                })
            }
        }
    };
    match executable {
        Executable::Instructions(instructions) => {
            instructions
                .iter()
                .enumerate()
                .find_map(|(instruction_index, instruction)| {
                    validate_one(instruction_index, instruction)
                })
        }
        Executable::IvmProved(proved) => {
            proved
                .overlay
                .iter()
                .enumerate()
                .find_map(|(instruction_index, instruction)| {
                    validate_one(instruction_index, instruction)
                })
        }
        Executable::Batch(items) => {
            items
                .iter()
                .enumerate()
                .find_map(|(instruction_index, item)| match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        validate_one(instruction_index, instruction)
                    }
                    ExecutableBatchItem::ContractCall(_) => None,
                })
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => None,
    }
}
fn invalid_sccp_record_instruction_in_signed_block(
    block: &SignedBlock,
) -> Option<SccpRecordInstructionValidationError> {
    for (entrypoint_index, entrypoint) in block.network_entrypoints().enumerate() {
        let Some(transaction) = signed_transaction_from_sccp_entrypoint(entrypoint) else {
            continue;
        };
        if !entrypoint_has_successful_or_pending_result(block, entrypoint_index) {
            continue;
        }
        if let Some(error) = invalid_sccp_record_instruction_in_executable(
            entrypoint_index,
            transaction.instructions(),
        ) {
            return Some(error);
        }
    }
    None
}
/// Discover direct SCCP records from successful Network sources or resultless proposal inputs.
///
/// Internal outputs are not network transactions. Malformed attached outputs yield no
/// candidates; acceptance must use the strict committed validator. This is not the
/// missing complete applied-outbox projection for callback-emitted records.
pub fn collect_sccp_messages_from_signed_block(block: &SignedBlock) -> Vec<RecordedSccpMessage> {
    collect_sccp_messages_from_signed_block_with_deduplication(block, true)
}
/// Return the first duplicate successful SCCP outbound key in a signed block, if any.
pub(crate) fn duplicate_sccp_outbound_message_key_in_signed_block(
    block: &SignedBlock,
) -> Option<SccpOutboundMessageKeyV1> {
    let mut seen = BTreeSet::new();
    for message in collect_sccp_messages_from_signed_block_with_deduplication(block, false) {
        let key =
            SccpOutboundMessageKeyV1::new(message.context.lane, message.commitment.message_id)?;
        if !seen.insert(key) {
            return Some(key);
        }
    }
    None
}
/// Validation error for committed SCCP records reconstructed from a signed block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SccpCommittedBlockValidationError {
    /// The block advertises an SCCP root but has no committed transaction results.
    MissingTransactionResults {
        /// Root advertised in the block header.
        actual: [u8; 32],
    },
    /// Proposal inputs, typed output ownership, or the retained output tree are malformed.
    InvalidExecutionOutputs(String),
    /// A successful or pending entrypoint contains a malformed SCCP record instruction.
    InvalidRecordInstruction(SccpRecordInstructionValidationError),
    /// The block contains more than one successful outbound message with the same replay key.
    DuplicateOutboundMessage(SccpOutboundMessageKeyV1),
    /// The block contains more successful outbound messages than the fixed first-release bound.
    TooManyOutboundMessages {
        /// Number of successful outbound messages reconstructed from the block.
        actual: usize,
        /// Maximum successful outbound messages admitted per block.
        max: usize,
    },
    /// The block header commitment root does not match reconstructed SCCP records.
    CommitmentRootMismatch {
        /// Root recomputed from committed SCCP message records.
        expected: Option<[u8; 32]>,
        /// Root advertised in the block header.
        actual: Option<[u8; 32]>,
    },
}
fn validate_sccp_execution_projection(block: &SignedBlock) -> Result<(), String> {
    block.validate_proposal_commitments()?;
    if block.has_results() {
        block
            .validate_output_merkle_cache()
            .map_err(|error| error.to_string())?;
        // Only directly submitted Network records have the legacy projection below.
        // TODO: use the one authenticated applied outbox for callback-emitted records;
        // neither a trace nor a trigger descriptor supplies its missing source authority.
        for output in block.execution_outputs() {
            if let Ok(steps) = output.result().as_ref()
                && steps.iter().any(|step| {
                    step.instructions.iter().any(|instruction| {
                        instruction
                            .as_any()
                            .is::<iroha_data_model::isi::bridge::RecordSccpMessage>()
                    })
                })
            {
                return Err(
                    "SCCP callback records require authenticated applied-outbox projection".into(),
                );
            }
        }
    }
    Ok(())
}
/// Validate committed SCCP records against the signed block header.
///
/// This check is intentionally fail-closed for duplicate successful outbound
/// keys before comparing roots, so a malformed block cannot be accepted by
/// signing a root over a deduplicated message list.
pub(crate) fn validate_sccp_commitment_root_for_signed_block(
    block: &SignedBlock,
) -> Result<(), SccpCommittedBlockValidationError> {
    if let Some(actual) = block.header().sccp_commitment_root()
        && !block.has_results()
    {
        return Err(SccpCommittedBlockValidationError::MissingTransactionResults { actual });
    }
    validate_sccp_execution_projection(block)
        .map_err(SccpCommittedBlockValidationError::InvalidExecutionOutputs)?;
    if let Some(error) = invalid_sccp_record_instruction_in_signed_block(block) {
        return Err(SccpCommittedBlockValidationError::InvalidRecordInstruction(
            error,
        ));
    }
    if let Some(key) = duplicate_sccp_outbound_message_key_in_signed_block(block) {
        return Err(SccpCommittedBlockValidationError::DuplicateOutboundMessage(
            key,
        ));
    }
    let messages = collect_sccp_messages_from_signed_block(block);
    let max = usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1)
        .expect("SCCP block bound fits usize");
    if messages.len() > max {
        return Err(SccpCommittedBlockValidationError::TooManyOutboundMessages {
            actual: messages.len(),
            max,
        });
    }
    let expected = sccp_commitment_root_from_messages(&messages);
    let actual = block.header().sccp_commitment_root();
    if actual == expected {
        Ok(())
    } else {
        Err(SccpCommittedBlockValidationError::CommitmentRootMismatch { expected, actual })
    }
}
/// Compute the SCCP commitment Merkle root for a set of recorded messages.
pub fn sccp_commitment_root_from_messages(messages: &[RecordedSccpMessage]) -> Option<[u8; 32]> {
    let commitments: Vec<_> = messages
        .iter()
        .map(|message| message.commitment.clone())
        .collect();
    iroha_sccp::commitment_merkle_root(&commitments)
}
/// Errors returned when constructing a bridge finality proof.
#[allow(variant_size_differences)]
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BridgeFinalityError {
    /// The requested block height is zero.
    #[error("invalid block height {0}")]
    InvalidHeight(u64),
    /// No durable Sumeragi-v2 finality artifact exists for the requested height.
    #[error("Sumeragi-v2 finality artifact for height {0} not found")]
    FinalityArtifactNotFound(u64),
    /// Kura could not decode or validate the durable artifact.
    #[error("failed to load Sumeragi-v2 finality artifact for height {height}: {reason}")]
    FinalityArtifactRead {
        /// Height being proven.
        height: u64,
        /// Bounded Kura validation diagnostic.
        reason: String,
    },
    /// The durable artifact does not match the selected block header or chain.
    #[error("Sumeragi-v2 finality artifact for height {height} does not match the selected block")]
    FinalityArtifactMismatch {
        /// Height being proven.
        height: u64,
    },
}
/// Build a self-contained finality proof for the block at `height`.
///
/// The proof bundles the block header and Kura's exact immutable v2 finality
/// artifact. The artifact owns BLS PoPs aligned with its frozen powered roster,
/// so historical verification never consults mutable validator state.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the height is zero, the durable retained-header artifact is
/// missing/malformed, or the exact v2 artifact fails cryptographic verification.
pub fn build_finality_proof(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BridgeFinalityProof, BridgeFinalityError> {
    if height == 0 {
        return Err(BridgeFinalityError::InvalidHeight(height));
    }
    let verified_finality = state
        .bridge_verified_v2_finality_artifact(height)
        .map_err(|reason| BridgeFinalityError::FinalityArtifactRead { height, reason })?
        .ok_or(BridgeFinalityError::FinalityArtifactNotFound(height))?;
    build_finality_proof_from_verified(state.bridge_network_id(), height, &verified_finality)
}
fn build_finality_proof_from_verified(
    network_id: &NetworkId,
    height: u64,
    verified_finality: &VerifiedV2FinalityArtifact,
) -> Result<BridgeFinalityProof, BridgeFinalityError> {
    let block_header = verified_finality.retained_header().clone();
    let finality_artifact = verified_finality.artifact().clone();
    if finality_artifact.height != height
        || block_header.height().get() != height
        || finality_artifact.height_context.network_id != *network_id
        || finality_artifact
            .validate_for_header(&block_header)
            .is_err()
    {
        return Err(BridgeFinalityError::FinalityArtifactMismatch { height });
    }
    Ok(BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header,
        finality_artifact,
    })
}
/// Build and sign one challenge-bound attestation for the exact committed state tip.
///
/// The first block hash and requested tip are taken from one immutable state view. The
/// embedded proof is loaded from Kura's verified finality boundary, and the reducer status
/// must name that exact proof before the node key is allowed to sign anything.
///
/// # Errors
///
/// Returns [`BridgeFinalityAttestationBuildError`] when the state is empty, the requested
/// height is not its exact tip, finality is unavailable, the status/proof body is inconsistent,
/// or the configured signer cannot produce a verifiable signature.
pub fn build_finality_attestation(
    state: &impl StateReadOnly,
    status: SumeragiV2Status,
    height: u64,
    challenge: [u8; 32],
    signer: &KeyPair,
) -> Result<BridgeFinalityAttestationV1, BridgeFinalityAttestationBuildError> {
    if signer.algorithm() != Algorithm::BlsNormal {
        return Err(BridgeFinalityAttestationBuildError::InvalidSignerAlgorithm);
    }
    let committed_height = u64::try_from(state.block_hashes().len())
        .map_err(|_| BridgeFinalityAttestationBuildError::HeightOverflow)?;
    if committed_height == 0 {
        return Err(BridgeFinalityAttestationBuildError::EmptyState);
    }
    require_exact_durable_tip_height(height, committed_height)?;
    let genesis_block_hash = state
        .block_hashes()
        .first()
        .copied()
        .ok_or(BridgeFinalityAttestationBuildError::EmptyState)?;
    let committed_tip_hash = state
        .block_hashes()
        .last()
        .copied()
        .ok_or(BridgeFinalityAttestationBuildError::EmptyState)?;
    let genesis_finality_proof = build_finality_proof(state, 1)
        .map_err(BridgeFinalityAttestationBuildError::GenesisFinalityProof)?;
    require_finality_proof_at_committed_genesis(
        genesis_block_hash,
        genesis_finality_proof.finality_artifact.block_hash,
    )?;
    let finality_proof = build_finality_proof(state, height)
        .map_err(BridgeFinalityAttestationBuildError::FinalityProof)?;
    require_finality_proof_at_committed_tip(
        committed_tip_hash,
        finality_proof.finality_artifact.block_hash,
    )?;
    let node_id = PeerId::new(signer.public_key().clone());
    let node_fingerprint = Hash::new(norito::codec::Encode::encode(&node_id));
    let body = BridgeFinalityAttestationBodyV1 {
        version: BRIDGE_FINALITY_ATTESTATION_VERSION_V1,
        challenge,
        network_id: *state.network_id(),
        node_id,
        node_fingerprint,
        genesis_block_hash,
        genesis_finality_proof,
        status,
        finality_proof,
    };
    body.validate_consistency()
        .map_err(BridgeFinalityAttestationBuildError::InvalidBody)?;
    let signature = SignatureOf::try_from_hash(signer.private_key(), body.signing_hash())
        .map_err(|error| BridgeFinalityAttestationBuildError::Signing(error.to_string()))?;
    let attestation = BridgeFinalityAttestationV1 { body, signature };
    attestation
        .verify()
        .map_err(BridgeFinalityAttestationBuildError::InvalidBody)?;
    Ok(attestation)
}
fn require_finality_proof_at_committed_tip(
    committed_tip_hash: iroha_crypto::HashOf<BlockHeader>,
    proof_block_hash: iroha_crypto::HashOf<BlockHeader>,
) -> Result<(), BridgeFinalityAttestationBuildError> {
    if proof_block_hash != committed_tip_hash {
        return Err(BridgeFinalityAttestationBuildError::FinalityTipMismatch {
            committed_tip_hash,
            proof_block_hash,
        });
    }
    Ok(())
}
fn require_exact_durable_tip_height(
    requested: u64,
    committed: u64,
) -> Result<(), BridgeFinalityAttestationBuildError> {
    if requested != committed {
        return Err(BridgeFinalityAttestationBuildError::HeightIsNotDurableTip {
            requested,
            committed,
        });
    }
    Ok(())
}
fn require_finality_proof_at_committed_genesis(
    committed_genesis_hash: iroha_crypto::HashOf<BlockHeader>,
    proof_block_hash: iroha_crypto::HashOf<BlockHeader>,
) -> Result<(), BridgeFinalityAttestationBuildError> {
    if proof_block_hash != committed_genesis_hash {
        return Err(
            BridgeFinalityAttestationBuildError::GenesisFinalityMismatch {
                committed_genesis_hash,
                proof_block_hash,
            },
        );
    }
    Ok(())
}
/// Failure while producing a node-signed durable-tip finality attestation.
#[derive(Debug, Error)]
pub enum BridgeFinalityAttestationBuildError {
    /// No committed genesis exists in the state snapshot.
    #[error("cannot attest finality for an empty state")]
    EmptyState,
    /// The committed block count cannot be represented on the wire.
    #[error("committed height does not fit into u64")]
    HeightOverflow,
    /// Only the exact durable tip may be attested.
    #[error("requested height {requested} is not durable tip {committed}")]
    HeightIsNotDurableTip {
        /// Requested block height.
        requested: u64,
        /// Exact committed state-view tip.
        committed: u64,
    },
    /// The verified finality record is not for the immutable state-view tip hash.
    #[error(
        "finality proof block hash {proof_block_hash:?} does not match committed tip {committed_tip_hash:?}"
    )]
    FinalityTipMismatch {
        /// Last block hash in the immutable state view.
        committed_tip_hash: iroha_crypto::HashOf<BlockHeader>,
        /// Block hash authenticated by the loaded finality proof.
        proof_block_hash: iroha_crypto::HashOf<BlockHeader>,
    },
    /// The verified height-one finality record is not for the immutable state-view genesis hash.
    #[error(
        "genesis finality proof block hash {proof_block_hash:?} does not match committed genesis {committed_genesis_hash:?}"
    )]
    GenesisFinalityMismatch {
        /// First block hash in the immutable state view.
        committed_genesis_hash: iroha_crypto::HashOf<BlockHeader>,
        /// Block hash authenticated by the loaded height-one proof.
        proof_block_hash: iroha_crypto::HashOf<BlockHeader>,
    },
    /// The production node signer is not the current BLS consensus identity.
    #[error("finality attestation signer must use BlsNormal")]
    InvalidSignerAlgorithm,
    /// Kura could not produce the exact verified proof for the requested tip.
    #[error("failed to build durable-tip finality proof: {0:?}")]
    FinalityProof(BridgeFinalityError),
    /// Kura could not produce the exact verified proof for committed height one.
    #[error("failed to build committed-genesis finality proof: {0:?}")]
    GenesisFinalityProof(BridgeFinalityError),
    /// Status, node identity, genesis, or proof duplicate bindings disagree.
    #[error("finality attestation body is inconsistent: {0}")]
    InvalidBody(BridgeFinalityAttestationValidationError),
    /// The configured private key failed to sign the domain-separated body hash.
    #[error("failed to sign finality attestation: {0}")]
    Signing(String),
}
/// Build an SCCP Groth16 request from a bundle bound to one already verified local artifact.
///
/// The marker is the trust boundary: Kura mints it after cache-backed verification, while
/// untrusted [`BridgeStateReadOnly`] providers must mint it with
/// [`VerifiedV2FinalityArtifact::verify_for_header`]. This function requires the bundle's
/// canonical finality proof to equal the marker's exact retained header and artifact before
/// delegating to SCCP's structural request assembler, so it never repeats BLS verification.
#[must_use]
pub fn build_sccp_groth16_bn254_proof_request_from_verified_finality_v1(
    verified_finality: &VerifiedV2FinalityArtifact,
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    let finality = TairaBridgeFinalityProofV1 {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header: verified_finality.retained_header().clone(),
        finality_artifact: verified_finality.artifact().clone(),
    };
    iroha_sccp::build_sccp_groth16_bn254_proof_request_from_structurally_bound_finality_v1(
        bundle,
        governed_route,
        &finality,
    )
}
/// Build the governed curve-specific SCCP destination proving request from an
/// already verified local finality artifact.
///
/// TON routes select BLS12-381; EVM and TRON routes select BN254.
/// The marker and exact embedded finality equality remain the trust boundary,
/// so this function performs no second Taira BLS verification.
#[must_use]
pub fn build_sccp_destination_proof_request_from_verified_finality_v1(
    verified_finality: &VerifiedV2FinalityArtifact,
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<iroha_sccp::SccpDestinationProofRequestV1> {
    let finality = TairaBridgeFinalityProofV1 {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header: verified_finality.retained_header().clone(),
        finality_artifact: verified_finality.artifact().clone(),
    };
    iroha_sccp::build_sccp_destination_proof_request_from_structurally_bound_finality_v1(
        bundle,
        governed_route,
        &finality,
    )
}
/// Fully authenticated finalized SCCP outbox projection for one exact block height.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSccpFinalizedMessagesV1 {
    verified_finality: VerifiedV2FinalityArtifact,
    /// Exact retained-header finality proof used to authenticate the projection.
    pub finality_proof: BridgeFinalityProof,
    /// Merkle root committed by the retained block header.
    pub commitment_root: [u8; 32],
    /// Canonical messages in zero-based commitment-index order.
    pub messages: Vec<ValidatedSccpOutboundMessageProjectionV1>,
}
impl ValidatedSccpFinalizedMessagesV1 {
    /// Borrow the exact cache-backed finality marker used to authenticate this projection.
    #[must_use]
    pub const fn verified_finality(&self) -> &VerifiedV2FinalityArtifact {
        &self.verified_finality
    }
}
/// Reconstruct and authenticate all finalized SCCP messages at one exact height.
///
/// The scan is bounded by [`iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1`]
/// and uses Kura's immutable, root-authenticated payload archive plus its exact retained-header
/// finality record. Historical block bodies and mutable WSV payloads are deliberately outside this
/// proof-serving boundary.
///
/// # Errors
///
/// Returns a bounded diagnostic when finality or the immutable archive is absent or malformed,
/// the projection is not dense and canonical, or its reconstructed root differs from the
/// finalized header.
pub fn validated_sccp_finalized_messages_at_height(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<Option<ValidatedSccpFinalizedMessagesV1>, String> {
    if height == 0 {
        return Err(BridgeFinalityError::InvalidHeight(height).to_string());
    }
    let (verified_finality, messages) = state
        .bridge_verified_v2_finality_with_sccp_archive(height)?
        .ok_or_else(|| BridgeFinalityError::FinalityArtifactNotFound(height).to_string())?;
    let finality_proof =
        build_finality_proof_from_verified(state.bridge_network_id(), height, &verified_finality)
            .map_err(|error| error.to_string())?;
    let Some((commitment_root, messages)) = validate_sccp_outbound_projection_against_root(
        height,
        finality_proof.block_header.sccp_commitment_root(),
        messages,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(ValidatedSccpFinalizedMessagesV1 {
        verified_finality,
        finality_proof,
        commitment_root,
        messages,
    }))
}
fn validate_sccp_outbound_projection_against_root(
    height: u64,
    anchored_root: Option<[u8; 32]>,
    messages: Vec<ValidatedSccpOutboundMessageProjectionV1>,
) -> Result<Option<([u8; 32], Vec<ValidatedSccpOutboundMessageProjectionV1>)>, String> {
    let max = usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1)
        .expect("SCCP block bound fits usize");
    if messages.len() > max {
        return Err(format!(
            "SCCP outbox projection at height {height} contains {} messages, exceeding the fixed {max}-message bound",
            messages.len()
        ));
    }
    let mut message_ids = BTreeSet::new();
    for (index, message) in messages.iter().enumerate() {
        let expected_index = u32::try_from(index).expect("bounded SCCP index fits u32");
        if message.commitment_index != expected_index {
            return Err(format!(
                "SCCP outbox projection at height {height} is not in dense commitment order: expected {expected_index}, found {}",
                message.commitment_index
            ));
        }
        if !message.context.is_well_formed()
            || iroha_sccp::hub_commitment_from_sccp_payload(message.context, &message.payload)
                .as_ref()
                != Some(&message.commitment)
        {
            return Err(format!(
                "SCCP outbox projection at height {height} contains a substituted context, payload, or commitment at index {expected_index}"
            ));
        }
        if !message_ids.insert(message.commitment.message_id) {
            return Err(format!(
                "SCCP outbox projection at height {height} repeats message identifier {}",
                hex::encode(message.commitment.message_id)
            ));
        }
    }
    if messages.is_empty() {
        return if anchored_root.is_none() {
            Ok(None)
        } else {
            Err(format!(
                "finalized SCCP header at height {height} commits a root but the immutable outbox archive is empty"
            ))
        };
    }
    let commitment_root = anchored_root.ok_or_else(|| {
        format!(
            "immutable SCCP outbox archive exists at height {height} but the retained finalized header has no commitment root"
        )
    })?;
    let commitments = messages
        .iter()
        .map(|message| message.commitment.clone())
        .collect::<Vec<_>>();
    let reconstructed = iroha_sccp::commitment_merkle_root(&commitments).ok_or_else(|| {
        format!("failed to reconstruct the bounded SCCP commitment root at height {height}")
    })?;
    if reconstructed != commitment_root {
        return Err(format!(
            "immutable SCCP outbox archive at height {height} reconstructs root 0x{}, expected finalized root 0x{}",
            hex::encode(reconstructed),
            hex::encode(commitment_root)
        ));
    }
    Ok(Some((commitment_root, messages)))
}
/// Build a compact commitment plus exact typed finality proof for `height`.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the underlying finality proof cannot be
/// built for the requested height.
pub fn build_finality_bundle(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BridgeFinalityBundle, BridgeFinalityError> {
    let proof = build_finality_proof(state, height)?;
    let commitment = BridgeCommitment {
        network_id: proof.finality_artifact.height_context.network_id,
        height_context_id: proof.finality_artifact.context_id(),
        block_height: proof.finality_artifact.height,
        block_hash: proof.finality_artifact.block_hash,
    };
    Ok(BridgeFinalityBundle {
        commitment,
        finality_proof: proof,
    })
}
/// Verification errors raised when checking a BridgeFinalityProof.
#[allow(variant_size_differences)]
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BridgeFinalityVerificationError {
    /// The caller expected a different finalized height.
    #[error("finality proof height mismatch: expected {expected}, actual {actual}")]
    HeightMismatch {
        /// Height requested by the caller.
        expected: u64,
        /// Height carried by the exact v2 artifact.
        actual: u64,
    },
    /// Exact proof verification failed.
    #[error(transparent)]
    Verification(#[from] iroha_data_model::bridge::BridgeFinalityVerifyError),
}
/// Verification knobs for verify_finality_proof.
#[derive(Debug, Clone, Copy)]
pub struct FinalityProofVerificationConfig<'a> {
    /// Exact genesis-derived network identity expected by the verifier.
    pub expected_network_id: &'a NetworkId,
    /// Optional expected height to bind the proof to a specific block.
    pub expected_height: Option<u64>,
    /// Trusted context id for the exact height being verified.
    pub trusted_context_id: iroha_data_model::block::consensus_v2::HeightContextId,
}
/// Verify a BridgeFinalityProof against network, height, context, powered quorum,
/// PoP, and aggregate-signature expectations.
///
/// # Errors
///
/// Returns BridgeFinalityVerificationError when the expected height differs or
/// the exact typed Sumeragi-v2 proof fails verification.
pub fn verify_finality_proof(
    proof: &BridgeFinalityProof,
    config: &FinalityProofVerificationConfig<'_>,
) -> Result<(), BridgeFinalityVerificationError> {
    if let Some(expected_height) = config.expected_height {
        let actual = proof.finality_artifact.height;
        if actual != expected_height {
            return Err(BridgeFinalityVerificationError::HeightMismatch {
                expected: expected_height,
                actual,
            });
        }
    }
    let mut verifier = iroha_data_model::bridge::BridgeFinalityVerifier::with_context(
        *config.expected_network_id,
        config.trusted_context_id,
    );
    verifier.verify(proof)?;
    Ok(())
}
#[cfg(test)]
fn validate_local_sccp_records_against_commitment_root(
    local_block: &SignedBlock,
    commitment_root: [u8; 32],
) -> Result<(), String> {
    if !local_block.has_results() {
        return Err(
            "SCCP finality proof local block is missing committed transaction results".to_owned(),
        );
    }
    validate_sccp_execution_projection(local_block).map_err(|reason| {
        format!("SCCP finality proof local block has invalid execution outputs: {reason}")
    })?;
    if let Some(error) = invalid_sccp_record_instruction_in_signed_block(local_block) {
        return Err(format!(
            "SCCP finality proof local block contains invalid outbound SCCP record: tx_index={} instruction_index={} reason={}",
            error.tx_index(),
            error.instruction_index(),
            error.reason()
        ));
    }
    if let Some(key) = duplicate_sccp_outbound_message_key_in_signed_block(local_block) {
        return Err(format!(
            "SCCP finality proof local block contains duplicate outbound message source_profile={} target_profile={} message_id={}",
            key.lane.source.profile_key(),
            key.lane.target.profile_key(),
            hex::encode(key.message_id)
        ));
    }
    let messages = collect_sccp_messages_from_signed_block(local_block);
    if sccp_commitment_root_from_messages(&messages) != Some(commitment_root) {
        return Err(
            "SCCP finality proof commitment root does not match local SCCP records".to_owned(),
        );
    }
    Ok(())
}
/// Verify an SCCP finality proof against local committed block and v2 artifact data.
///
/// This intentionally rejects proofs when the local node cannot load the committed block or
/// exact durable Sumeragi-v2 artifact for the referenced height.
///
/// # Errors
/// Returns a human-readable rejection reason when the SCCP proof is not anchored to local state
/// or when the trusted local artifact fails full finality verification.
pub fn verify_sccp_finality_proof_against_local_state(
    state: &impl BridgeStateReadOnly,
    finality: &TairaBridgeFinalityProofV1,
) -> Result<BridgeFinalityProof, String> {
    if !iroha_sccp::verify_taira_bridge_finality_proof_structure(finality) {
        return Err("SCCP finality proof failed structural verification".to_owned());
    }
    verify_structural_sccp_finality_proof_against_local_state(state, finality)
}
/// Bind a parse-only destination proof to local committed block and durable v2
/// artifact state before any settlement-call material can be derived.
///
/// # Errors
/// Returns a human-readable rejection reason when the context's finality artifact differs from
/// authoritative local state or the proof-typed storage lookup rejects that local artifact.
pub fn verify_sccp_parsed_destination_proof_against_local_state(
    state: &impl BridgeStateReadOnly,
    parsed: &iroha_sccp::SccpParsedDestinationProofV1,
) -> Result<BridgeFinalityProof, String> {
    verify_structural_sccp_finality_proof_against_local_state(state, parsed.finality())
}
fn verify_structural_sccp_finality_proof_against_local_state(
    state: &impl BridgeStateReadOnly,
    finality: &TairaBridgeFinalityProofV1,
) -> Result<BridgeFinalityProof, String> {
    let artifact = &finality.finality_artifact;
    let height = artifact.height;
    if artifact.height_context.network_id != *state.bridge_network_id() {
        return Err("SCCP finality proof network id does not match local state".to_owned());
    }
    let local = validated_sccp_finalized_messages_at_height(state, height)?
        .ok_or_else(|| format!("local finalized block at height {height} has no SCCP messages"))?;
    if local.finality_proof.block_header != finality.block_header {
        return Err(
            "SCCP finality proof block header does not match the retained local canonical header"
                .to_owned(),
        );
    }
    if local.finality_proof.finality_artifact != *artifact {
        return Err(
            "SCCP finality proof artifact does not match the exact durable local artifact"
                .to_owned(),
        );
    }
    Ok(finality.clone())
}

/// One replay admission reconstructed only from commit-authenticated execution
/// input and its successful result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SccpCommittedReplayAdmissionV1 {
    /// Exact route/boundary forest selected by the committed instruction.
    pub accumulator_id: SccpReplayAccumulatorIdV1,
    /// Complete replay domain independently derived from committed fields.
    pub domain: SccpReplayDomainV1,
    /// Semantic occupied-leaf record independently derived from committed fields.
    pub record: SccpReplayRecordV1,
    /// Exact caller witness that made the committed transition possible.
    pub witness: iroha_data_model::bridge::SccpSparseMerkleWitnessV1,
}

/// Failure while reconstructing replay state from finalized Kura execution.
///
/// Variants intentionally retain no proof bytes, identities, paths, or parser
/// diagnostics so an archive service cannot reflect attacker-controlled data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SccpReplayRebuildErrorV1 {
    /// A finalized block is absent or its entrypoint/result commitment is malformed.
    MalformedBlock,
    /// A referenced merge execution is absent, misaligned, or malformed.
    MalformedMerge,
    /// A successful SCCP instruction cannot produce the unique canonical record.
    MalformedAdmission,
    /// One signed transaction or trigger call claims more than one replay mutation.
    MultipleMutations,
    /// A committed mutation names a forest absent from the supplied final registry projection.
    UnknownAccumulator,
    /// An archive transition or final rebuilt forest differs from Core consensus state.
    ForestMismatch,
    /// The selected finalized Kura tip changed during the scan.
    FinalityChanged,
}

impl core::fmt::Display for SccpReplayRebuildErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::MalformedBlock => "malformed finalized SCCP replay block projection",
            Self::MalformedMerge => "malformed finalized SCCP merge projection",
            Self::MalformedAdmission => "malformed committed SCCP replay admission",
            Self::MultipleMutations => "multiple SCCP replay mutations in one execution call",
            Self::UnknownAccumulator => "committed SCCP replay accumulator is not registered",
            Self::ForestMismatch => "rebuilt SCCP replay forest differs from consensus state",
            Self::FinalityChanged => "finalized Kura tip changed during SCCP replay rebuild",
        })
    }
}

impl std::error::Error for SccpReplayRebuildErrorV1 {}

fn canonical_replay_route_parts(
    transfer: &iroha_sccp::TransferPayloadV1,
) -> Result<(&str, &str), SccpReplayRebuildErrorV1> {
    if transfer.route_id_codec != iroha_sccp::SCCP_CODEC_CANONICAL_TEXT
        || transfer.asset_id_codec != iroha_sccp::SCCP_CODEC_CANONICAL_TEXT
    {
        return Err(SccpReplayRebuildErrorV1::MalformedAdmission);
    }
    let route_id = core::str::from_utf8(&transfer.route_id)
        .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?;
    let asset_key = core::str::from_utf8(&transfer.asset_id)
        .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?;
    if route_id.is_empty()
        || asset_key.is_empty()
        || asset_key.contains('#')
        || asset_key.parse::<Name>().is_err()
    {
        return Err(SccpReplayRebuildErrorV1::MalformedAdmission);
    }
    Ok((route_id, asset_key))
}

fn replay_route_key(
    lane: SccpLaneIdV1,
    transfer: &iroha_sccp::TransferPayloadV1,
) -> Result<SccpRouteKeyV1, SccpReplayRebuildErrorV1> {
    let (route_id, asset_key) = canonical_replay_route_parts(transfer)?;
    SccpRouteKeyV1::new(
        lane,
        route_id.to_owned(),
        asset_key.to_owned(),
        transfer.route_revision,
    )
    .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)
}

fn committed_outbound_replay_admission(
    authority: &iroha_data_model::account::AccountId,
    record: &iroha_data_model::isi::bridge::RecordSccpMessage,
) -> Result<SccpCommittedReplayAdmissionV1, SccpReplayRebuildErrorV1> {
    let validated =
        validate_recorded_sccp_message_payload_bytes(record.context, &record.payload_bytes)
            .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?;
    let SccpPayloadV1::Transfer(transfer) = &validated.payload;
    let route_key = replay_route_key(
        SccpLaneIdV1 {
            source: validated.context.lane.target,
            target: validated.context.lane.source,
        },
        transfer,
    )?;
    let boundary = SccpReplayBoundaryV1::SoraOutboundLock;
    let domain = SccpReplayDomainV1 {
        source_network: validated.context.lane.source,
        target_network: validated.context.lane.target,
        boundary,
        route_revision: transfer.route_revision,
        route_configuration_hash: validated.context.route_configuration_hash,
        actor: SccpReplayActorV1::Route,
    };
    let accumulator_id = SccpReplayAccumulatorIdV1::from_domain(route_key, &domain)
        .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?;
    Ok(SccpCommittedReplayAdmissionV1 {
        accumulator_id,
        domain,
        record: SccpReplayRecordV1 {
            operation: boundary,
            replay_id: validated.key.message_id,
            payload_sha256: sha2::Sha256::digest(&record.payload_bytes).into(),
            amount: transfer.amount,
            principal: SccpReplayPrincipalV1::SoraAccount(authority.clone()),
            auxiliary_identity_sha256: sha2::Sha256::digest(
                validated.context.destination_binding_hash,
            )
            .into(),
        },
        witness: record.replay_witness.clone(),
    })
}

fn canonical_taira_recipient_for_rebuild(
    transfer: &iroha_sccp::TransferPayloadV1,
) -> Result<iroha_data_model::account::AccountId, SccpReplayRebuildErrorV1> {
    if transfer.recipient_codec != iroha_sccp::SCCP_CODEC_CANONICAL_TEXT {
        return Err(SccpReplayRebuildErrorV1::MalformedAdmission);
    }
    let literal = core::str::from_utf8(&transfer.recipient)
        .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?;
    let address = iroha_data_model::account::AccountAddress::parse_encoded(
        literal,
        Some(iroha_sccp::SCCP_TAIRA_I105_DISCRIMINANT_V1),
    )
    .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?;
    if address
        .to_i105_for_discriminant(iroha_sccp::SCCP_TAIRA_I105_DISCRIMINANT_V1)
        .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?
        != literal
    {
        return Err(SccpReplayRebuildErrorV1::MalformedAdmission);
    }
    let account = address
        .to_account_id()
        .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?;
    if account
        .try_signatory()
        .is_none_or(|key| key.algorithm() != Algorithm::Ed25519)
    {
        return Err(SccpReplayRebuildErrorV1::MalformedAdmission);
    }
    Ok(account)
}

fn committed_native_replay_admission(
    submit: &iroha_data_model::isi::bridge::SubmitBridgeProof,
) -> Result<Option<SccpCommittedReplayAdmissionV1>, SccpReplayRebuildErrorV1> {
    let iroha_data_model::bridge::BridgeProofPayload::NativeProtocol(native) =
        &submit.proof.payload
    else {
        return Ok(None);
    };
    let witness = submit
        .replay_witness
        .clone()
        .ok_or(SccpReplayRebuildErrorV1::MalformedAdmission)?;
    let decoded = iroha_sccp::decode_bridge_native_protocol_proof_v1(native)
        .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?;
    let SccpPayloadV1::Transfer(transfer) = &decoded.payload;
    if decoded.source.lane.source.domain_id() != transfer.source_domain
        || decoded.source.lane.target.domain_id() != transfer.dest_domain
        || transfer.dest_domain != iroha_sccp::SCCP_DOMAIN_SORA
        || transfer.asset_home_domain != iroha_sccp::SCCP_DOMAIN_SORA
    {
        return Err(SccpReplayRebuildErrorV1::MalformedAdmission);
    }
    let route_key = replay_route_key(decoded.source.lane, transfer)?;
    let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&decoded.payload)
        .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?;
    let boundary = SccpReplayBoundaryV1::SoraInboundRelease;
    let domain = SccpReplayDomainV1 {
        source_network: decoded.source.lane.source,
        target_network: decoded.source.lane.target,
        boundary,
        route_revision: transfer.route_revision,
        route_configuration_hash: native.route_configuration_hash,
        actor: SccpReplayActorV1::Route,
    };
    let accumulator_id = SccpReplayAccumulatorIdV1::from_domain(route_key, &domain)
        .map_err(|_| SccpReplayRebuildErrorV1::MalformedAdmission)?;
    Ok(Some(SccpCommittedReplayAdmissionV1 {
        accumulator_id,
        domain,
        record: SccpReplayRecordV1 {
            operation: boundary,
            replay_id: decoded.source.message_id,
            payload_sha256: sha2::Sha256::digest(payload_bytes).into(),
            amount: transfer.amount,
            principal: SccpReplayPrincipalV1::SoraAccount(canonical_taira_recipient_for_rebuild(
                transfer,
            )?),
            auxiliary_identity_sha256: sha2::Sha256::digest(decoded.source.source_event_digest)
                .into(),
        },
        witness,
    }))
}

fn collect_replay_admissions_from_instruction_step<I>(
    authority: Option<&iroha_data_model::account::AccountId>,
    instructions: I,
    proved_overlay: bool,
) -> Result<Vec<SccpCommittedReplayAdmissionV1>, SccpReplayRebuildErrorV1>
where
    I: IntoIterator,
    I::Item: std::borrow::Borrow<InstructionBox>,
{
    let mut admissions = Vec::new();
    for instruction in instructions {
        let instruction = std::borrow::Borrow::borrow(&instruction);
        let any = instruction.as_any();
        if let Some(record) = any.downcast_ref::<iroha_data_model::isi::bridge::RecordSccpMessage>()
        {
            if !proved_overlay {
                return Err(SccpReplayRebuildErrorV1::MalformedAdmission);
            }
            admissions.push(committed_outbound_replay_admission(
                authority.ok_or(SccpReplayRebuildErrorV1::MalformedAdmission)?,
                record,
            )?);
        }
        if let Some(submit) = any.downcast_ref::<iroha_data_model::isi::bridge::SubmitBridgeProof>()
            && let Some(admission) = committed_native_replay_admission(submit)?
        {
            admissions.push(admission);
        }
    }
    if admissions.len() > 1 {
        return Err(SccpReplayRebuildErrorV1::MultipleMutations);
    }
    Ok(admissions)
}

fn collect_initial_replay_admissions(
    entrypoint: &TransactionEntrypoint,
) -> Result<Vec<SccpCommittedReplayAdmissionV1>, SccpReplayRebuildErrorV1> {
    match entrypoint {
        TransactionEntrypoint::External(transaction) => collect_replay_admissions_from_executable(
            transaction.authority(),
            transaction.instructions(),
        ),
        TransactionEntrypoint::SealedReveal(reveal) => {
            let transaction = reveal.signed_transaction();
            collect_replay_admissions_from_executable(
                transaction.authority(),
                transaction.instructions(),
            )
        }
        TransactionEntrypoint::SealedCommitment(_) => Ok(Vec::new()),
    }
}

fn collect_replay_admissions_from_executable(
    authority: &iroha_data_model::account::AccountId,
    executable: &Executable,
) -> Result<Vec<SccpCommittedReplayAdmissionV1>, SccpReplayRebuildErrorV1> {
    match executable {
        Executable::Instructions(instructions) => collect_replay_admissions_from_instruction_step(
            Some(authority),
            instructions.iter(),
            false,
        ),
        Executable::Batch(items) => collect_replay_admissions_from_instruction_step(
            Some(authority),
            items.iter().filter_map(|item| match item {
                ExecutableBatchItem::Instruction(instruction) => Some(instruction),
                ExecutableBatchItem::ContractCall(_) => None,
            }),
            false,
        ),
        Executable::IvmProved(proved) => collect_replay_admissions_from_instruction_step(
            Some(authority),
            proved.overlay.iter(),
            true,
        ),
        Executable::ContractCall(_) | Executable::Ivm(_) => Ok(Vec::new()),
    }
}

fn collect_replay_admissions_from_entrypoint_result(
    entrypoint: &TransactionEntrypoint,
    result: &TransactionResult,
) -> Result<Vec<SccpCommittedReplayAdmissionV1>, SccpReplayRebuildErrorV1> {
    let Ok(data_triggers) = result.as_ref() else {
        return Ok(Vec::new());
    };
    let mut admissions = collect_initial_replay_admissions(entrypoint)?;
    for step in data_triggers {
        admissions.extend(collect_replay_admissions_from_instruction_step(
            None,
            step.instructions.iter(),
            false,
        )?);
    }
    if admissions.len() > 1 {
        return Err(SccpReplayRebuildErrorV1::MultipleMutations);
    }
    Ok(admissions)
}

/// Extract replay admissions in persisted merge-then-canonical-output order.
///
/// An authenticated merge batch is consumed before ordinary carrier
/// Network outputs. A Network row joins its exact source through `input_index`;
/// successful Pipeline/Time rows contribute their one root-first instruction trace
/// without becoming network inputs. Rejected executions contribute no mutation.
/// This projector preserves the proved-overlay restriction on outbound admission;
/// it does not supply the missing applied-outbox authority for internal records.
/// TODO: the applied-outbox owner must authenticate execution ordering where it
/// differs from Network source order (for example reordered sealed reveals).
pub fn collect_sccp_replay_admissions_from_finalized_execution(
    block: &SignedBlock,
    merge_entry: Option<&iroha_data_model::merge::MergeLedgerEntry>,
) -> Result<Vec<SccpCommittedReplayAdmissionV1>, SccpReplayRebuildErrorV1> {
    block
        .validate_output_merkle_cache()
        .map_err(|_| SccpReplayRebuildErrorV1::MalformedBlock)?;
    let mut admissions = Vec::new();
    if let Some(batch) = merge_entry.and_then(|entry| entry.execution_batch.as_ref()) {
        if batch.version != 1 || !crate::merge::merge_execution_batch_commitments_match(batch) {
            return Err(SccpReplayRebuildErrorV1::MalformedMerge);
        }
        let observed = batch
            .lanes
            .iter()
            .try_fold(0_usize, |count, lane| {
                if lane.entrypoints.len() != lane.results.len() {
                    return None;
                }
                count.checked_add(lane.entrypoints.len())
            })
            .ok_or(SccpReplayRebuildErrorV1::MalformedMerge)?;
        if u64::try_from(observed).ok() != Some(batch.entrypoint_count) {
            return Err(SccpReplayRebuildErrorV1::MalformedMerge);
        }
        for lane in &batch.lanes {
            for (entrypoint, result) in lane.entrypoints.iter().zip(&lane.results) {
                admissions.extend(collect_replay_admissions_from_entrypoint_result(
                    entrypoint, result,
                )?);
            }
        }
    }
    for output in block.execution_outputs() {
        match output {
            ExecutionOutputV1::Network(row) => {
                let entrypoint = block
                    .network_entrypoint_at(
                        usize::try_from(row.input_index)
                            .map_err(|_| SccpReplayRebuildErrorV1::MalformedBlock)?,
                    )
                    .ok_or(SccpReplayRebuildErrorV1::MalformedBlock)?;
                admissions.extend(collect_replay_admissions_from_entrypoint_result(
                    entrypoint,
                    &row.result,
                )?);
            }
            ExecutionOutputV1::Pipeline(_) | ExecutionOutputV1::Time(_) => {
                let Ok(steps) = output.result().as_ref() else {
                    continue;
                };
                let mut invocation = Vec::new();
                for step in steps {
                    invocation.extend(collect_replay_admissions_from_instruction_step(
                        None,
                        step.instructions.iter(),
                        false,
                    )?);
                }
                if invocation.len() > 1 {
                    return Err(SccpReplayRebuildErrorV1::MultipleMutations);
                }
                admissions.extend(invocation);
            }
        }
    }
    Ok(admissions)
}

/// Rebuild all supplied replay forests by scanning finalized Kura blocks and
/// authenticated merge execution, then require byte-exact agreement with Core.
///
/// `expected` must be projected from the final Core registry/state snapshot at
/// `finalized_height`. It preinitializes every accumulator, including empty
/// routes, and prevents a mutation from inventing an accumulator or domain.
pub fn rebuild_sccp_replay_archive_from_kura_v1(
    kura: &crate::kura::Kura,
    finalized_height: NonZeroUsize,
    expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
) -> Result<SccpReplayArchiveV1, SccpReplayRebuildErrorV1> {
    if kura.blocks_count() < finalized_height.get() {
        return Err(SccpReplayRebuildErrorV1::MalformedBlock);
    }
    let tip_hash = kura
        .get_block_hash(finalized_height)
        .ok_or(SccpReplayRebuildErrorV1::MalformedBlock)?;
    let mut archive = SccpReplayArchiveV1::default();
    for (accumulator_id, (domain, _)) in expected {
        archive
            .initialize_accumulator(accumulator_id.clone(), *domain)
            .map_err(|_| SccpReplayRebuildErrorV1::UnknownAccumulator)?;
    }
    for height in 1..=finalized_height.get() {
        let height = NonZeroUsize::new(height).expect("range begins at one");
        let block = kura
            .get_block(height)
            .ok_or(SccpReplayRebuildErrorV1::MalformedBlock)?;
        if usize::try_from(block.header().height().get()).ok() != Some(height.get()) {
            return Err(SccpReplayRebuildErrorV1::MalformedBlock);
        }
        let merge_entry = kura
            .get_merge_entry_by_carrier_height(height)
            .map_err(|_| SccpReplayRebuildErrorV1::MalformedMerge)?;
        for admission in
            collect_sccp_replay_admissions_from_finalized_execution(&block, merge_entry.as_ref())?
        {
            let (expected_domain, _) = expected
                .get(&admission.accumulator_id)
                .ok_or(SccpReplayRebuildErrorV1::UnknownAccumulator)?;
            if expected_domain != &admission.domain {
                return Err(SccpReplayRebuildErrorV1::UnknownAccumulator);
            }
            archive
                .apply_record(
                    admission.accumulator_id,
                    &admission.record,
                    &admission.witness,
                )
                .map_err(|_| SccpReplayRebuildErrorV1::ForestMismatch)?;
        }
    }
    if kura.get_block_hash(finalized_height) != Some(tip_hash) {
        return Err(SccpReplayRebuildErrorV1::FinalityChanged);
    }
    for (accumulator_id, (domain, forest)) in expected {
        let (rebuilt_domain, rebuilt_forest) = archive
            .forest(accumulator_id)
            .map_err(|_| SccpReplayRebuildErrorV1::UnknownAccumulator)?;
        if &rebuilt_domain != domain || rebuilt_forest != forest {
            return Err(SccpReplayRebuildErrorV1::ForestMismatch);
        }
    }
    Ok(archive)
}
#[cfg(test)]
mod tests;
