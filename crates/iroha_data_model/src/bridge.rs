//! Bridge-related data types for wrapped assets and receipts.
//! Feature-gated behind `bridge`.

use std::{string::String, vec::Vec};

use iroha_crypto::PublicKey;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    ChainId, consensus::VALIDATOR_SET_HASH_VERSION_V1, nexus::LaneId, peer::PeerId, proof::ProofBox,
};

/// Versioned SCCP network, lane, and source-identity wire types.
pub mod sccp;
mod sccp_registry;
pub use sccp::{
    SCCP_V1_JSON_SAFE_INTEGER_MAX, SccpEvmSourceEmitterV1, SccpInboundAnchorHighWaterKeyV1,
    SccpInboundMessageKeyV1, SccpInboundMessageRecordV1, SccpLaneIdV1, SccpNetworkV1,
    SccpOutboundMessageContextV1, SccpOutboundMessageIndexKeyV1, SccpOutboundMessageKeyV1,
    SccpOutboundMessageRecordV1, SccpOutboundProofRecordV1, SccpSourceEmitterV1,
    SccpSourceIdentityV1, SccpTronSourceEmitterV1,
};
pub use sccp_registry::{
    SCCP_V1_MAX_GOVERNED_LANES, SCCP_V1_MAX_KEY_BYTES, SCCP_V1_MAX_LIVE_GOVERNED_ROUTES,
    SCCP_V1_MAX_LIVE_ROUTES_PER_LANE, SCCP_V1_MAX_PAYLOAD_AMOUNT_SCALE,
    SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE, SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE,
    SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER, SCCP_V1_TAIRA_XOR_ASSET_DEFINITION_ID,
    SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE, SccpBn254G1PointV1, SccpBn254G2PointV1,
    SccpDestinationDeploymentV1, SccpEvmDestinationDeploymentV1, SccpGovernedLaneV1,
    SccpGovernedRouteV1, SccpGroth16Bn254IcV1, SccpGroth16Bn254SemanticCircuitV1,
    SccpGroth16Bn254VerifyingKeyV1, SccpInboundFinalityCutoffV1, SccpOutboundProofPolicyV1,
    SccpRegistryV1, SccpRouteActivationV1, SccpRouteKeyV1, SccpRouteValidationError,
    SccpSemanticProofProfileV1, SccpSoraFinalityAnchorV1, SccpSoraSettlementV1,
    SccpTronDestinationDeploymentV1, canonical_sccp_groth16_bn254_public_signal_schema_bytes_v1,
    canonical_sccp_groth16_bn254_verifying_key_bytes_v1, canonical_sccp_lane_id_bytes_v1,
    canonical_sccp_network_bytes_v1, canonical_sccp_semantic_proof_profile_bytes_v1,
    canonical_sccp_sora_finality_anchor_bytes_v1, canonical_sccp_source_emitter_bytes_v1,
    canonical_sccp_source_identity_bytes_v1, sccp_evm_destination_binding_hash_v1,
    sccp_exact_evm_xor_route_config_hash_v1, sccp_exact_tron_xor_route_config_hash_v1,
    sccp_groth16_bn254_public_signal_schema_hash_v1, sccp_groth16_bn254_verifying_key_hash_v1,
    sccp_lane_id_hash_v1, sccp_network_identity_hash_v1, sccp_network_tag_v1,
    sccp_semantic_proof_profile_hash_v1, sccp_sora_finality_anchor_hash_v1,
    sccp_sora_taira_chain_id_hash_v1, sccp_source_emitter_identity_hash_v1,
    sccp_source_identity_hash_v1, sccp_tron_destination_binding_hash_v1,
    sccp_v1_taira_xor_asset_definition_id,
};

/// Definition metadata for a wrapped asset originating from another chain.
///
/// Stored alongside an Iroha asset definition to bind it to its origin.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct WrappedAssetDef {
    /// Origin chain identifier (canonical bytes, e.g., "btc", "evm-eth").
    pub origin_chain: Vec<u8>,
    /// Origin asset identifier on the origin chain (canonical bytes).
    pub origin_asset_id: Vec<u8>,
    /// Bridge lane identifier that minted this wrapped asset (canonical bytes).
    pub bridge_id: Vec<u8>,
}

/// A receipt emitted by the bridge lane to record a cross-chain action.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeReceipt {
    /// Lane identifier (e.g., "btc→iroha", "iroha↔evm").
    pub lane: LaneId,
    /// Direction of the action: "lock", "mint", "burn", or "release".
    pub direction: Vec<u8>,
    /// Source transaction or message hash (32 bytes canonical).
    pub source_tx: [u8; 32],
    /// Optional destination transaction hash, if known.
    pub dest_tx: Option<[u8; 32]>,
    /// Hash of the verification proof submitted for this action.
    pub proof_hash: [u8; 32],
    /// Amount transferred (integer units matching the asset definition).
    pub amount: u128,
    /// Canonical Iroha asset id bytes.
    pub asset_id: Vec<u8>,
    /// Recipient identifier bytes (Iroha account id or external address payload).
    pub recipient: Vec<u8>,
}

/// Hash function used by bridge Merkle proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
#[norito(tag = "hash_function", content = "value")]
pub enum BridgeHashFunction {
    /// SHA-256 (ICS-style hash-only light clients).
    Sha256,
    /// Blake2b (mirrors Iroha’s internal hash).
    Blake2b,
}

/// Height range covered by a bridge proof artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeProofRange {
    /// Inclusive start height of the batch.
    pub start_height: u64,
    /// Inclusive end height of the batch.
    pub end_height: u64,
}

impl BridgeProofRange {
    /// Returns `true` if the range is non-empty and ordered.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.start_height <= self.end_height
    }

    /// Length of the covered window (`end_height - start_height + 1`).
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.end_height
            .saturating_sub(self.start_height)
            .saturating_add(1)
    }

    /// Returns `true` when the range is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// ICS-style proof payload (hash-only light client).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeIcsProof {
    /// Exact verifier manifest commitment selected for this proof.
    pub verifier_manifest_hash: [u8; 32],
    /// State root advertised by the counterparty chain.
    pub state_root: [u8; 32],
    /// Leaf hash being proven.
    pub leaf_hash: [u8; 32],
    /// Compact Merkle path from leaf to root.
    pub proof: iroha_crypto::MerkleProof<[u8; 32]>,
    /// Hash function used when computing parent nodes.
    pub hash_function: BridgeHashFunction,
}

/// Transparent ZK proof payload (rolling recursive proof).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeTransparentProof {
    /// Exact verifier manifest commitment selected for this proof.
    pub verifier_manifest_hash: [u8; 32],
    /// Opaque proof bytes tagged with backend identifier.
    pub proof: ProofBox,
    /// Optional recursion depth claimed by the prover.
    pub recursion_depth: Option<u32>,
}

/// Closed protocol-native backend identifiers for first-release SCCP proofs.
///
/// Unlike a transparent proof backend, this identifier is not a caller-chosen
/// string. Each value selects one concrete native consensus and inclusion
/// verifier, so an unknown value fails decoding instead of being routed by a
/// node-local naming convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "backend", content = "protocol")]
pub enum BridgeNativeProofBackendV1 {
    /// Ethereum proof using the beacon light client and execution MPTs.
    #[codec(index = 0)]
    #[norito(rename = "ethereum_beacon_v1")]
    EthereumBeacon,
    /// BNB Smart Chain proof using native Parlia finality and execution MPTs.
    #[codec(index = 1)]
    #[norito(rename = "bsc_parlia_v1")]
    BscParlia,
    /// TRON proof using native DPoS replay and transaction inclusion.
    #[codec(index = 2)]
    #[norito(rename = "tron_dpos_v1")]
    TronDpos,
}

impl BridgeNativeProofBackendV1 {
    /// Return the stable, unambiguous bridge backend label.
    #[must_use]
    pub const fn backend_label(self) -> &'static str {
        match self {
            Self::EthereumBeacon => "bridge/sccp/native/ethereum-beacon-v1",
            Self::BscParlia => "bridge/sccp/native/bsc-parlia-v1",
            Self::TronDpos => "bridge/sccp/native/tron-dpos-v1",
        }
    }

    /// Return whether V1 admits this backend for the exact source-network profile.
    ///
    /// The closed first-release inventory contains only the three verifier
    /// families with complete value-moving implementations.
    #[must_use]
    pub const fn supports_source_network(self, source: SccpNetworkV1) -> bool {
        matches!(
            (self, source),
            (
                Self::EthereumBeacon,
                SccpNetworkV1::EthereumMainnet | SccpNetworkV1::EthereumSepolia
            ) | (
                Self::BscParlia,
                SccpNetworkV1::BscMainnet | SccpNetworkV1::BscTestnet
            ) | (
                Self::TronDpos,
                SccpNetworkV1::TronMainnet | SccpNetworkV1::TronNile | SccpNetworkV1::TronShasta
            )
        )
    }
}

/// Governed protocol-native trust anchor for one SCCP lane.
///
/// `anchor_hash` is interpreted only by the closed `backend` verifier. Keeping
/// the family tag beside the commitment prevents a valid checkpoint hash from
/// being routed to a different chain verifier through a domain-only lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct SccpNativeTrustAnchorV1 {
    /// Concrete native verifier that defines the anchor preimage and hash.
    pub backend: BridgeNativeProofBackendV1,
    /// Nonzero, role-separated hash of the governed native checkpoint.
    pub anchor_hash: [u8; 32],
    /// Backend-specific consensus-progress coordinate committed by `anchor_hash`.
    ///
    /// Ethereum lanes use a finalized beacon slot. BSC and TRON lanes use a
    /// finalized block height. This is intentionally distinct from an
    /// Ethereum execution-block height carried by an admitted event proof.
    pub checkpoint_height: u64,
}

impl SccpNativeTrustAnchorV1 {
    /// Return whether the trust anchor contains a nonzero commitment.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        self.anchor_hash.iter().any(|byte| *byte != 0) && self.checkpoint_height != 0
    }

    /// Return whether an authenticated consensus-progress coordinate belongs
    /// to this anchor's governance interval.
    ///
    /// The next retained checkpoint is an inclusive upper boundary. The
    /// one-height overlap lets BSC/TRON prove the boundary block while the
    /// successor checkpoint itself becomes usable. Without a successor the
    /// current checkpoint remains open-ended.
    #[must_use]
    pub fn admits_anchor_interval_height(
        self,
        anchor_interval_height: u64,
        inclusive_successor_boundary: Option<u64>,
    ) -> bool {
        anchor_interval_height >= self.checkpoint_height
            && inclusive_successor_boundary.is_none_or(|upper| anchor_interval_height <= upper)
    }
}

/// Canonically encoded SCCP protocol-native admission envelope.
///
/// The SCCP crate owns and validates the typed envelope because it owns the
/// chain-specific verifier DTOs. The data model stores that canonical encoding
/// once, paired with a closed backend identifier; it does not disguise native
/// consensus evidence as a transparent ZK proof or place it inside a
/// caller-labelled [`ProofBox`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeNativeProtocolProofV1 {
    /// Concrete native verifier selected for the encoded envelope.
    pub backend: BridgeNativeProofBackendV1,
    /// Immutable governed route configuration authenticated by the envelope.
    pub route_configuration_hash: [u8; 32],
    /// Canonical Norito bytes of the typed SCCP native inbound proof.
    pub encoded_envelope: Vec<u8>,
}

impl BridgeNativeProtocolProofV1 {
    /// Return whether the container carries a nonzero route commitment and a
    /// nonempty canonical-envelope candidate.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.route_configuration_hash.iter().any(|byte| *byte != 0)
            && !self.encoded_envelope.is_empty()
    }
}

/// Closed production destination verifier selected for an SCCP artifact.
///
/// An unknown or caller-labelled backend is unrepresentable. The SCCP
/// cryptographic implementation additionally verifies that the canonical
/// artifact's inner family agrees with this outer tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "backend", content = "family")]
pub enum BridgeSccpDestinationProofBackendV1 {
    /// EVM Groth16 verifier over BN254 for Ethereum and BSC destinations.
    #[codec(index = 0)]
    #[norito(rename = "evm_groth16_bn254_v1")]
    EvmGroth16Bn254,
    /// TVM Groth16 verifier over BN254 for TRON destinations.
    #[codec(index = 1)]
    #[norito(rename = "tron_groth16_bn254_v1")]
    TronGroth16Bn254,
}

impl BridgeSccpDestinationProofBackendV1 {
    /// Return the stable production verifier label used in proof diagnostics.
    #[must_use]
    pub const fn backend_label(self) -> &'static str {
        match self {
            Self::EvmGroth16Bn254 => "evm-groth16-bn254-v1",
            Self::TronGroth16Bn254 => "tron-groth16-bn254-v1",
        }
    }
}

/// Canonically encoded production SCCP destination-proof artifact.
///
/// This closed container prevents production SCCP delivery from being routed
/// through generic [`ProofBox`] backend strings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct BridgeSccpDestinationProofV1 {
    /// Closed production verifier selected for the encoded artifact.
    pub backend: BridgeSccpDestinationProofBackendV1,
    /// Immutable governed route configuration authenticated by the artifact.
    ///
    /// Keeping this commitment in the typed payload means historical route
    /// rotation cannot reinterpret a proof envelope.
    pub route_configuration_hash: [u8; 32],
    /// Canonical Norito bytes of the typed SCCP destination artifact.
    pub encoded_artifact: Vec<u8>,
}

impl BridgeSccpDestinationProofV1 {
    /// Return whether the closed proof carries nonempty artifact bytes and an
    /// independently named route-configuration commitment.
    #[must_use]
    pub fn is_well_formed_for(
        &self,
        destination_binding_hash: [u8; 32],
        artifact_commitment: [u8; 32],
    ) -> bool {
        self.route_configuration_hash.iter().any(|byte| *byte != 0)
            && destination_binding_hash.iter().any(|byte| *byte != 0)
            && artifact_commitment.iter().any(|byte| *byte != 0)
            && self.route_configuration_hash != destination_binding_hash
            && self.route_configuration_hash != artifact_commitment
            && destination_binding_hash != artifact_commitment
            && !self.encoded_artifact.is_empty()
    }
}

/// Bridge proof payload kinds supported by the data model.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
#[norito(tag = "kind", content = "payload")]
pub enum BridgeProofPayload {
    /// ICS-23-style inclusion proof against a state root.
    #[codec(index = 0)]
    Ics(BridgeIcsProof),
    /// Transparent recursive ZK proof.
    #[codec(index = 1)]
    TransparentZk(BridgeTransparentProof),
    /// Protocol-native SCCP consensus and message-inclusion proof.
    #[codec(index = 2)]
    NativeProtocol(BridgeNativeProtocolProofV1),
    /// Closed production proof for delivering an SORA-origin SCCP message.
    #[codec(index = 3)]
    SccpDestination(BridgeSccpDestinationProofV1),
}

/// Typed verifier binding computed from a bridge proof payload.
///
/// This value is not stored independently in [`BridgeProof`]. Keeping the
/// commitment beside the payload that defines its meaning makes it impossible
/// to reinterpret a route-configuration hash as a generic verifier manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BridgeProofBinding {
    /// Commitment to the verifier manifest used by a generic proof backend.
    VerifierManifest([u8; 32]),
    /// Commitment to the exact historical SCCP route configuration.
    SccpRouteConfigurationV1([u8; 32]),
}

impl BridgeProofBinding {
    /// Return the bound commitment bytes.
    #[must_use]
    pub const fn hash(self) -> [u8; 32] {
        match self {
            Self::VerifierManifest(hash) | Self::SccpRouteConfigurationV1(hash) => hash,
        }
    }

    /// Return whether the binding carries a nonzero commitment.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        self.hash().iter().any(|byte| *byte != 0)
    }
}

impl BridgeProofPayload {
    /// Return the role-preserving verifier binding carried by this payload.
    #[must_use]
    pub const fn binding(&self) -> BridgeProofBinding {
        match self {
            Self::Ics(proof) => BridgeProofBinding::VerifierManifest(proof.verifier_manifest_hash),
            Self::TransparentZk(proof) => {
                BridgeProofBinding::VerifierManifest(proof.verifier_manifest_hash)
            }
            Self::NativeProtocol(proof) => {
                BridgeProofBinding::SccpRouteConfigurationV1(proof.route_configuration_hash)
            }
            Self::SccpDestination(proof) => {
                BridgeProofBinding::SccpRouteConfigurationV1(proof.route_configuration_hash)
            }
        }
    }
}

/// Bridge proof artifact with a payload-owned verifier binding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeProof {
    /// Height range covered by this proof.
    pub range: BridgeProofRange,
    /// Proof payload (generic ICS/ZK or one of the two closed SCCP proof roles).
    pub payload: BridgeProofPayload,
}

impl BridgeProof {
    /// Return the role-preserving verifier binding carried by the payload.
    #[must_use]
    pub const fn binding(&self) -> BridgeProofBinding {
        self.payload.binding()
    }

    /// Return a backend label suitable for hashing/id construction.
    #[must_use]
    pub fn backend_label(&self) -> String {
        match &self.payload {
            BridgeProofPayload::Ics(_) => "bridge/ics23".to_owned(),
            BridgeProofPayload::TransparentZk(p) => {
                format!("bridge/{}", p.proof.backend)
            }
            BridgeProofPayload::NativeProtocol(p) => p.backend.backend_label().to_owned(),
            BridgeProofPayload::SccpDestination(p) => p.backend.backend_label().to_owned(),
        }
    }
}

/// Stored bridge proof record with size metadata and commitment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeProofRecord {
    /// Recorded proof artifact.
    pub proof: BridgeProof,
    /// Hash commitment for the proof bytes (backend-specific).
    pub commitment: [u8; 32],
    /// Total encoded size of the stored proof (bytes).
    pub size_bytes: u32,
}

/// Finality proof for an Iroha block built from the consensus commit certificate.
///
/// This proof is self-contained: it carries the block header, its hash, and the
/// commit certificate (validator set + BLS aggregate signature) produced by the
/// active validator set for that height. Verifiers recompute the block hash from
/// the header and validate the commit certificate aggregate signature against the
/// provided validator set and the certificate's mode tag.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeFinalityProof {
    /// Height of the finalized block.
    pub height: u64,
    /// Chain identifier to prevent cross-chain replay.
    pub chain_id: crate::ChainId,
    /// Block header for the finalized block.
    pub block_header: crate::block::BlockHeader,
    /// Consensus hash of the block header.
    pub block_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
    /// Commit certificate collected for the block.
    pub commit_qc: crate::consensus::Qc,
    /// Proof-of-possession entries aligned with `commit_qc.validator_set`.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub validator_set_pops: Vec<Vec<u8>>,
}

/// Authority set snapshot used for bridge commitments.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeAuthoritySet {
    /// Monotonically increasing authority set identifier.
    pub id: u64,
    /// Ordered validator set at this authority set id.
    pub validator_set: Vec<crate::peer::PeerId>,
    /// Hash of the validator set using the configured hash version.
    pub validator_set_hash: iroha_crypto::HashOf<Vec<crate::peer::PeerId>>,
    /// Hash version used when computing `validator_set_hash`.
    pub validator_set_hash_version: u16,
}

/// Commitment covering a block hash and authority set.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeCommitment {
    /// Chain identifier to prevent cross-chain replay.
    pub chain_id: crate::ChainId,
    /// Authority set that signed this commitment.
    pub authority_set: BridgeAuthoritySet,
    /// Block height bound into the commitment.
    pub block_height: u64,
    /// Block hash bound into the commitment (used as the leaf hash in the MMR).
    pub block_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
    /// Optional MMR root covering recent blocks. When present, verifiers should
    /// prefer MMR inclusion proofs over direct hash checks.
    pub mmr_root: Option<[u8; 32]>,
    /// Optional leaf index in the MMR for this block (0-based).
    pub mmr_leaf_index: Option<u64>,
    /// Optional list of MMR peaks associated with `mmr_root` to help external
    /// verifiers reconstruct the root without replaying the full chain.
    ///
    /// Peaks are ordered from left to right (in insertion order). When
    /// reconstructing the root, bag peaks from right to left:
    /// `root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`.
    pub mmr_peaks: Option<Vec<[u8; 32]>>,
    /// Optional next authority set advertised by this commitment.
    pub next_authority_set: Option<BridgeAuthoritySet>,
}

/// Justification (signatures) for a bridge commitment.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeCommitmentJustification {
    /// Signatures from the authority set over the commitment payload.
    pub signatures: Vec<crate::block::BlockSignature>,
}

/// Bundle containing a commitment, justification, and block details.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct BridgeFinalityBundle {
    /// Commitment binding the block hash and authority set.
    pub commitment: BridgeCommitment,
    /// Justification (signatures) for the commitment.
    pub justification: BridgeCommitmentJustification,
    /// Block header for the finalized block.
    pub block_header: crate::block::BlockHeader,
    /// Commit certificate for the block.
    pub commit_qc: crate::consensus::Qc,
}

/// Errors surfaced when verifying bridge finality proofs.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BridgeFinalityVerifyError {
    /// Proof is bound to a different chain id.
    #[error("chain id mismatch: expected {expected}, got {got}")]
    ChainIdMismatch {
        /// Expected chain id.
        expected: ChainId,
        /// Chain id carried inside the proof.
        got: ChainId,
    },
    /// Commit certificate height disagrees with the proof height.
    #[error(
        "commit certificate height {certificate_height} does not match proof height {proof_height}"
    )]
    CertificateHeightMismatch {
        /// Height recorded in the proof.
        proof_height: u64,
        /// Height recorded in the commit certificate.
        certificate_height: u64,
    },
    /// Commit certificate phase is not `Commit`.
    #[error("commit certificate phase {got:?} does not match expected {expected:?}")]
    CertificatePhaseMismatch {
        /// Expected commit-certificate phase.
        expected: crate::block::consensus::CertPhase,
        /// Phase carried in the commit certificate.
        got: crate::block::consensus::CertPhase,
    },
    /// Block hash is inconsistent across the header/proof/certificate tuple.
    #[error(
        "block hash mismatch (header {header_hash:?}, proof field {proof_hash:?}, certificate {certificate_hash:?})"
    )]
    BlockHashMismatch {
        /// Hash recomputed from the block header.
        header_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
        /// Hash advertised by the proof.
        proof_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
        /// Hash advertised by the commit certificate.
        certificate_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
    },
    /// Validator-set hash version is unknown.
    #[error("validator set hash version {version} is not supported")]
    UnsupportedValidatorSetHashVersion {
        /// Validator-set hash version carried in the proof.
        version: u16,
    },
    /// Validator-set hash does not match the recorded validator set.
    #[error(
        "validator set hash mismatch: recorded {recorded:?}, computed {computed:?} (version {version})"
    )]
    ValidatorSetHashMismatch {
        /// Hash recorded in the proof.
        recorded: iroha_crypto::HashOf<Vec<PeerId>>,
        /// Hash recomputed from the validator set.
        computed: iroha_crypto::HashOf<Vec<PeerId>>,
        /// Validator-set hash version recorded in the proof.
        version: u16,
    },
    /// Proof was built with a different validator set than the expected anchor.
    #[error("validator set hash {got:?} does not match expected {expected:?}")]
    UnexpectedValidatorSet {
        /// Expected validator-set hash anchor.
        expected: iroha_crypto::HashOf<Vec<PeerId>>,
        /// Validator-set hash carried in the proof.
        got: iroha_crypto::HashOf<Vec<PeerId>>,
    },
    /// Proof was produced for a different epoch than the expected anchor.
    #[error("commit certificate epoch {got} does not match expected {expected}")]
    UnexpectedEpoch {
        /// Expected epoch anchor.
        expected: u64,
        /// Epoch carried in the proof.
        got: u64,
    },
    /// Verification attempted without an explicit validator-set hash anchor.
    #[error("validator set anchor is required before verifying bridge finality proofs")]
    MissingValidatorSetAnchor,
    /// Verification attempted without an explicit epoch anchor.
    #[error("epoch anchor is required before verifying bridge finality proofs")]
    MissingEpochAnchor,
    /// Proof carries an empty validator set.
    #[error("validator set is empty")]
    EmptyValidatorSet,
    /// Validator-set `PoP` length does not match the validator-set length.
    #[error("validator set pop length {got} does not match expected {expected}")]
    ValidatorSetPopLengthMismatch {
        /// Expected `PoP` count.
        expected: usize,
        /// Actual `PoP` count.
        got: usize,
    },
    /// Signer bitmap length does not match the validator-set length.
    #[error("signer bitmap length {got} does not match expected {expected}")]
    SignerBitmapLengthMismatch {
        /// Expected bitmap length.
        expected: usize,
        /// Actual bitmap length.
        got: usize,
    },
    /// Signer bitmap references a validator outside the roster bounds.
    #[error("signer index {index} is out of range for validator set length {len}")]
    SignatureIndexOutOfRange {
        /// Index inferred from the signer bitmap.
        index: u64,
        /// Validator-set length.
        len: usize,
    },
    /// Validator key is not a BLS key, so aggregate verification cannot proceed.
    #[error("validator key at index {index} is not BLS: {algorithm:?}")]
    InvalidValidatorKeyAlgorithm {
        /// Signer index that failed validation.
        index: u64,
        /// Algorithm advertised by the public key.
        algorithm: iroha_crypto::Algorithm,
    },
    /// Validator key compact state is malformed.
    #[error("validator key at index {index} is malformed")]
    MalformedValidatorPublicKey {
        /// Signer index that failed validation.
        index: u64,
    },
    /// Aggregate signature is missing from the commit certificate.
    #[error("aggregate signature is missing")]
    AggregateSignatureMissing,
    /// Aggregate signature failed to verify against the advertised validator set.
    #[error("aggregate signature failed to verify")]
    InvalidAggregateSignature,
    /// Proof does not contain enough signers to satisfy quorum.
    #[error("insufficient signers: required {required}, collected {collected}")]
    InsufficientSigners {
        /// Quorum required for the advertised validator set.
        required: usize,
        /// Unique signer count from the bitmap.
        collected: usize,
    },
    /// Proof height is older than the latest verified height.
    #[error("proof height {height} is stale relative to latest verified height {latest}")]
    StaleHeight {
        /// Latest height accepted by the verifier.
        latest: u64,
        /// Height carried by the proof.
        height: u64,
    },
    /// Proof height skips past the next expected height.
    #[error("proof height {height} advances past the next expected height after {latest}")]
    AdvancedHeight {
        /// Latest height accepted by the verifier.
        latest: u64,
        /// Height carried by the proof.
        height: u64,
    },
}

/// Stateful verifier for bridge finality proofs.
///
/// The verifier enforces the canonical `(block_header, block_hash, commit_qc)` tuple,
/// binds proofs to a chain id, and checks the commit-certificate aggregate signature against the
/// advertised validator set with the production quorum rule. It tracks the latest verified height
/// to reject stale or skipped proofs, and requires explicit validator-set and
/// epoch anchors to reject replays across topology changes.
#[derive(Debug, Clone)]
pub struct BridgeFinalityVerifier {
    expected_chain_id: ChainId,
    expected_validator_set_hash: Option<iroha_crypto::HashOf<Vec<PeerId>>>,
    validator_set_hash_version: u16,
    expected_epoch: Option<u64>,
    latest_height: Option<u64>,
}

impl BridgeFinalityVerifier {
    /// Construct a verifier bound to the expected `chain_id`.
    #[must_use]
    pub fn new(expected_chain_id: ChainId) -> Self {
        Self {
            expected_chain_id,
            expected_validator_set_hash: None,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            expected_epoch: None,
            latest_height: None,
        }
    }

    /// Construct a verifier bound to the expected `chain_id` and validator-set hash anchor.
    #[must_use]
    pub fn with_validator_set(
        expected_chain_id: ChainId,
        validator_set_hash: iroha_crypto::HashOf<Vec<PeerId>>,
        validator_set_hash_version: u16,
    ) -> Self {
        Self {
            expected_chain_id,
            expected_validator_set_hash: Some(validator_set_hash),
            validator_set_hash_version,
            expected_epoch: None,
            latest_height: None,
        }
    }

    /// Construct a verifier bound to the expected `chain_id`, validator-set hash, and epoch anchor.
    #[must_use]
    pub fn with_validator_set_and_epoch(
        expected_chain_id: ChainId,
        validator_set_hash: iroha_crypto::HashOf<Vec<PeerId>>,
        validator_set_hash_version: u16,
        expected_epoch: u64,
    ) -> Self {
        Self {
            expected_chain_id,
            expected_validator_set_hash: Some(validator_set_hash),
            validator_set_hash_version,
            expected_epoch: Some(expected_epoch),
            latest_height: None,
        }
    }

    /// Update the expected validator-set hash anchor used when verifying proofs.
    pub fn set_validator_set_anchor(
        &mut self,
        validator_set_hash: iroha_crypto::HashOf<Vec<PeerId>>,
        validator_set_hash_version: u16,
    ) {
        self.expected_validator_set_hash = Some(validator_set_hash);
        self.validator_set_hash_version = validator_set_hash_version;
    }

    /// Update the expected epoch anchor used when verifying proofs.
    pub fn set_epoch_anchor(&mut self, expected_epoch: u64) {
        self.expected_epoch = Some(expected_epoch);
    }

    /// Update both the validator-set and epoch anchors together to reflect a topology change.
    pub fn set_validator_set_and_epoch_anchor(
        &mut self,
        validator_set_hash: iroha_crypto::HashOf<Vec<PeerId>>,
        validator_set_hash_version: u16,
        expected_epoch: u64,
    ) {
        self.set_validator_set_anchor(validator_set_hash, validator_set_hash_version);
        self.expected_epoch = Some(expected_epoch);
    }

    /// Verify a bridge finality proof against the configured expectations.
    ///
    /// # Errors
    /// Returns [`BridgeFinalityVerifyError`] when the proof's chain id, height continuity,
    /// hashes, epoch anchor, validator-set hash/version, or commit signatures are invalid.
    pub fn verify(&mut self, proof: &BridgeFinalityProof) -> Result<(), BridgeFinalityVerifyError> {
        if proof.chain_id != self.expected_chain_id {
            return Err(BridgeFinalityVerifyError::ChainIdMismatch {
                expected: self.expected_chain_id.clone(),
                got: proof.chain_id.clone(),
            });
        }

        if let Some(latest) = self.latest_height {
            if proof.height <= latest {
                return Err(BridgeFinalityVerifyError::StaleHeight {
                    latest,
                    height: proof.height,
                });
            }
            if proof.height > latest.saturating_add(1) {
                return Err(BridgeFinalityVerifyError::AdvancedHeight {
                    latest,
                    height: proof.height,
                });
            }
        }

        if proof.commit_qc.height != proof.height {
            return Err(BridgeFinalityVerifyError::CertificateHeightMismatch {
                proof_height: proof.height,
                certificate_height: proof.commit_qc.height,
            });
        }
        if proof.commit_qc.phase != crate::block::consensus::CertPhase::Commit {
            return Err(BridgeFinalityVerifyError::CertificatePhaseMismatch {
                expected: crate::block::consensus::CertPhase::Commit,
                got: proof.commit_qc.phase,
            });
        }

        let header_hash = proof.block_header.hash();
        let proof_hash = proof.block_hash;
        let certificate_hash = proof.commit_qc.subject_block_hash;
        if header_hash != proof_hash || header_hash != certificate_hash {
            return Err(BridgeFinalityVerifyError::BlockHashMismatch {
                header_hash,
                proof_hash,
                certificate_hash,
            });
        }

        let recorded_version = proof.commit_qc.validator_set_hash_version;
        if recorded_version != self.validator_set_hash_version {
            return Err(
                BridgeFinalityVerifyError::UnsupportedValidatorSetHashVersion {
                    version: recorded_version,
                },
            );
        }

        let recorded_hash = proof.commit_qc.validator_set_hash;
        let computed_hash = iroha_crypto::HashOf::new(&proof.commit_qc.validator_set);
        if computed_hash != recorded_hash {
            return Err(BridgeFinalityVerifyError::ValidatorSetHashMismatch {
                recorded: recorded_hash,
                computed: computed_hash,
                version: recorded_version,
            });
        }

        let validator_set = &proof.commit_qc.validator_set;
        if validator_set.is_empty() {
            return Err(BridgeFinalityVerifyError::EmptyValidatorSet);
        }

        Self::validate_commit_qc(&proof.chain_id, &proof.commit_qc, &proof.validator_set_pops)?;

        let expected_epoch = self
            .expected_epoch
            .ok_or(BridgeFinalityVerifyError::MissingEpochAnchor)?;
        if proof.commit_qc.epoch != expected_epoch {
            return Err(BridgeFinalityVerifyError::UnexpectedEpoch {
                expected: expected_epoch,
                got: proof.commit_qc.epoch,
            });
        }

        let expected = self
            .expected_validator_set_hash
            .ok_or(BridgeFinalityVerifyError::MissingValidatorSetAnchor)?;
        if recorded_hash != expected {
            return Err(BridgeFinalityVerifyError::UnexpectedValidatorSet {
                expected,
                got: recorded_hash,
            });
        }

        self.latest_height = Some(proof.height);
        Ok(())
    }

    fn validate_commit_qc(
        chain_id: &ChainId,
        certificate: &crate::consensus::Qc,
        validator_set_pops: &[Vec<u8>],
    ) -> Result<(), BridgeFinalityVerifyError> {
        let validator_set = &certificate.validator_set;
        let required = Self::min_signatures(validator_set.len());
        let indices =
            signer_indices_from_bitmap(&certificate.aggregate.signers_bitmap, validator_set.len())?;
        let collected = indices.len();
        if collected < required {
            return Err(BridgeFinalityVerifyError::InsufficientSigners {
                required,
                collected,
            });
        }

        if certificate.aggregate.bls_aggregate_signature.is_empty() {
            return Err(BridgeFinalityVerifyError::AggregateSignatureMissing);
        }
        if validator_set_pops.len() != validator_set.len() {
            return Err(BridgeFinalityVerifyError::ValidatorSetPopLengthMismatch {
                expected: validator_set.len(),
                got: validator_set_pops.len(),
            });
        }

        let mut public_keys: Vec<&PublicKey> = Vec::with_capacity(indices.len());
        let mut pops: Vec<&[u8]> = Vec::with_capacity(indices.len());
        for idx in indices {
            let peer = &validator_set[idx];
            let algorithm = peer.public_key.try_algorithm().map_err(|_| {
                BridgeFinalityVerifyError::MalformedValidatorPublicKey { index: idx as u64 }
            })?;
            if algorithm != iroha_crypto::Algorithm::BlsNormal {
                return Err(BridgeFinalityVerifyError::InvalidValidatorKeyAlgorithm {
                    index: idx as u64,
                    algorithm,
                });
            }
            public_keys.push(peer.public_key());
            pops.push(validator_set_pops[idx].as_slice());
        }

        let preimage = commit_vote_preimage(chain_id, certificate);
        iroha_crypto::bls_normal_verify_preaggregated_same_message(
            &preimage,
            &certificate.aggregate.bls_aggregate_signature,
            &public_keys,
            &pops,
        )
        .map_err(|_| BridgeFinalityVerifyError::InvalidAggregateSignature)?;

        Ok(())
    }

    const fn min_signatures(len: usize) -> usize {
        if len <= 3 {
            len
        } else {
            len.saturating_mul(2) / 3 + 1
        }
    }
}

fn consensus_domain(
    chain_id: &ChainId,
    message_type_tag: &str,
    extra: &[u8],
    mode_tag: &str,
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, Digest as _};
    let mut hasher = Blake2b512::new();
    iroha_crypto::blake2::digest::Update::update(&mut hasher, b"iroha-sumeragi-consensus/v1");
    iroha_crypto::blake2::digest::Update::update(
        &mut hasher,
        chain_id.clone().into_inner().as_bytes(),
    );
    iroha_crypto::blake2::digest::Update::update(&mut hasher, mode_tag.as_bytes());
    iroha_crypto::blake2::digest::Update::update(
        &mut hasher,
        &crate::block::consensus::PROTO_VERSION.to_be_bytes(),
    );
    iroha_crypto::blake2::digest::Update::update(&mut hasher, message_type_tag.as_bytes());
    iroha_crypto::blake2::digest::Update::update(&mut hasher, extra);
    let digest = iroha_crypto::blake2::Digest::finalize(hasher);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn commit_vote_preimage(chain_id: &ChainId, certificate: &crate::consensus::Qc) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 32 * 4 + 8 * 6 + 3);
    let domain = consensus_domain(chain_id, "Vote", b"v2", &certificate.mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(certificate.subject_block_hash.as_ref().as_ref());
    out.extend_from_slice(certificate.parent_state_root.as_ref());
    out.extend_from_slice(certificate.post_state_root.as_ref());
    out.extend_from_slice(&certificate.height.to_be_bytes());
    out.extend_from_slice(&certificate.view.to_be_bytes());
    out.extend_from_slice(&certificate.epoch.to_be_bytes());
    out.extend_from_slice(certificate.chain_order_hash.as_ref());
    out.extend_from_slice(&certificate.rechain_seq.to_be_bytes());
    out.push(certificate.phase as u8);
    match certificate.highest_qc {
        Some(highest_qc) => {
            out.push(1);
            out.extend_from_slice(&highest_qc.height.to_be_bytes());
            out.extend_from_slice(&highest_qc.view.to_be_bytes());
            out.extend_from_slice(&highest_qc.epoch.to_be_bytes());
            out.extend_from_slice(highest_qc.subject_block_hash.as_ref().as_ref());
            out.push(highest_qc.phase as u8);
        }
        None => out.push(0),
    }

    out
}

fn signer_indices_from_bitmap(
    bitmap: &[u8],
    roster_len: usize,
) -> Result<Vec<usize>, BridgeFinalityVerifyError> {
    let expected_len = roster_len.div_ceil(8);
    if bitmap.len() != expected_len {
        return Err(BridgeFinalityVerifyError::SignerBitmapLengthMismatch {
            expected: expected_len,
            got: bitmap.len(),
        });
    }

    let mut indices = Vec::new();
    for (byte_idx, byte) in bitmap.iter().enumerate() {
        if *byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if (byte >> bit) & 1 == 0 {
                continue;
            }
            let idx = byte_idx * 8 + bit;
            if idx >= roster_len {
                return Err(BridgeFinalityVerifyError::SignatureIndexOutOfRange {
                    index: idx as u64,
                    len: roster_len,
                });
            }
            indices.push(idx);
        }
    }

    Ok(indices)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_version::DecodeAll;

    use super::*;

    fn validator_set_from_keys(keys: &[KeyPair]) -> Vec<PeerId> {
        keys.iter()
            .map(|kp| PeerId::from(kp.public_key().clone()))
            .collect()
    }

    fn signer_bitmap(len: usize, signers: &[usize]) -> Vec<u8> {
        let bytes = len.div_ceil(8);
        let mut bitmap = vec![0u8; bytes];
        for &idx in signers {
            let byte_idx = idx / 8;
            let bit = idx % 8;
            bitmap[byte_idx] |= 1u8 << bit;
        }
        bitmap
    }

    fn checked_commit_vote_signature_payload(keypair: &KeyPair, preimage: &[u8]) -> Vec<u8> {
        Signature::try_new(keypair.private_key(), preimage)
            .expect("checked bridge commit-vote signature")
            .payload()
            .to_vec()
    }

    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("test fixture random key generation should succeed")
    }

    fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm).unwrap_or_else(|err| {
            panic!("{algorithm:?} bridge fixture key generation should succeed: {err}")
        })
    }

    fn checked_bls_keypair() -> KeyPair {
        checked_random_keypair_with_algorithm(Algorithm::BlsNormal)
    }

    fn make_finality_proof_with_signers_and_mode(
        chain_id: &str,
        height: u64,
        epoch: u64,
        keys: &[KeyPair],
        signer_indices: &[usize],
        mode_tag: &str,
    ) -> BridgeFinalityProof {
        assert!(
            !signer_indices.is_empty(),
            "test helper requires at least one signer"
        );

        let header = crate::block::BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let block_hash = header.hash();
        let validator_set = validator_set_from_keys(keys);
        let validator_set_pops: Vec<Vec<u8>> = keys
            .iter()
            .map(|kp| {
                iroha_crypto::bls_normal_pop_prove(kp.private_key())
                    .expect("PoP prove for validator keypair")
            })
            .collect();
        let validator_set_hash = HashOf::new(&validator_set);
        let cert_template = crate::consensus::Qc {
            phase: crate::block::consensus::CertPhase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height,
            view: 0,
            epoch,
            chain_order_hash: crate::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: mode_tag.to_string(),
            highest_qc: None,
            validator_set_hash,
            validator_set_hash_version: crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            aggregate: crate::consensus::QcAggregate {
                signers_bitmap: Vec::new(),
                bls_aggregate_signature: Vec::new(),
            },
        };
        let preimage = commit_vote_preimage(&chain_id.parse().expect("chain id"), &cert_template);
        let mut sig_payloads = Vec::with_capacity(signer_indices.len());
        for &idx in signer_indices {
            sig_payloads.push(checked_commit_vote_signature_payload(&keys[idx], &preimage));
        }
        let sig_refs: Vec<&[u8]> = sig_payloads.iter().map(Vec::as_slice).collect();
        let aggregate =
            iroha_crypto::bls_normal_aggregate_signatures(&sig_refs).expect("aggregate signatures");
        let signers_bitmap = signer_bitmap(validator_set.len(), signer_indices);
        let commit_qc = crate::consensus::Qc {
            aggregate: crate::consensus::QcAggregate {
                signers_bitmap,
                bls_aggregate_signature: aggregate,
            },
            ..cert_template
        };

        BridgeFinalityProof {
            height,
            chain_id: chain_id.parse().expect("chain id"),
            block_header: header,
            block_hash,
            commit_qc,
            validator_set_pops,
        }
    }

    fn make_finality_proof(
        chain_id: &str,
        height: u64,
        epoch: u64,
        keys: &[KeyPair],
    ) -> BridgeFinalityProof {
        make_finality_proof_with_signers_and_mode(
            chain_id,
            height,
            epoch,
            keys,
            &(0..keys.len()).collect::<Vec<_>>(),
            crate::block::consensus::PERMISSIONED_TAG,
        )
    }

    fn verifier_anchored_to(proof: &BridgeFinalityProof) -> BridgeFinalityVerifier {
        BridgeFinalityVerifier::with_validator_set_and_epoch(
            proof.chain_id.clone(),
            proof.commit_qc.validator_set_hash,
            proof.commit_qc.validator_set_hash_version,
            proof.commit_qc.epoch,
        )
    }

    fn make_block_signature(
        index: u64,
        keypair: &KeyPair,
        header: &crate::block::BlockHeader,
    ) -> crate::block::BlockSignature {
        crate::block::BlockSignature::new(
            index,
            SignatureOf::try_from_hash(keypair.private_key(), header.hash())
                .expect("checked bridge block-header signature"),
        )
    }

    #[test]
    fn bridge_finality_fixture_checked_signature_verifies_preimage() {
        let keys = vec![checked_bls_keypair()];
        let proof = make_finality_proof("chain-a", 1, 0, &keys);
        let preimage = commit_vote_preimage(&proof.chain_id, &proof.commit_qc);
        let signature_payload = checked_commit_vote_signature_payload(&keys[0], &preimage);
        let signature = Signature::try_from_bytes(&signature_payload)
            .expect("checked bridge commit-vote signature payload passes admission");

        signature
            .verify(keys[0].public_key(), &preimage)
            .expect("checked bridge commit-vote signature verifies preimage");
    }

    #[test]
    fn bridge_proof_range_helpers_cover_valid_invalid_and_saturating_cases() {
        let valid = BridgeProofRange {
            start_height: 5,
            end_height: 7,
        };
        assert!(valid.is_valid());
        assert_eq!(valid.len(), 3);
        assert!(!valid.is_empty());

        let invalid = BridgeProofRange {
            start_height: 9,
            end_height: 4,
        };
        assert!(!invalid.is_valid());
        assert_eq!(invalid.len(), 1);
        assert!(!invalid.is_empty());

        let saturated = BridgeProofRange {
            start_height: u64::MAX,
            end_height: u64::MAX,
        };
        assert!(saturated.is_valid());
        assert_eq!(saturated.len(), 1);
    }

    #[test]
    fn bridge_proof_backend_label_matches_payload_kind() {
        let leaves = vec![[0xA1; 32], [0xB2; 32]];
        let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
        let root_bytes: [u8; 32] = *tree.root().expect("root").as_ref();
        let ics = BridgeProof {
            range: BridgeProofRange {
                start_height: 1,
                end_height: 1,
            },
            payload: BridgeProofPayload::Ics(BridgeIcsProof {
                verifier_manifest_hash: [0x11; 32],
                state_root: root_bytes,
                leaf_hash: leaves[0],
                proof: tree.get_proof(0).expect("proof"),
                hash_function: BridgeHashFunction::Sha256,
            }),
        };
        assert_eq!(ics.backend_label(), "bridge/ics23");

        let transparent = BridgeProof {
            range: BridgeProofRange {
                start_height: 2,
                end_height: 3,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: [0x22; 32],
                proof: ProofBox::new("halo2/mock".into(), vec![0xDE, 0xAD, 0xBE, 0xEF]),
                recursion_depth: Some(2),
            }),
        };
        assert_eq!(transparent.backend_label(), "bridge/halo2/mock");

        let native = BridgeProof {
            range: BridgeProofRange {
                start_height: 4,
                end_height: 4,
            },
            payload: BridgeProofPayload::NativeProtocol(BridgeNativeProtocolProofV1 {
                backend: BridgeNativeProofBackendV1::TronDpos,
                route_configuration_hash: [0x33; 32],
                encoded_envelope: vec![0x01, 0x02, 0x03],
            }),
        };
        assert_eq!(native.backend_label(), "bridge/sccp/native/tron-dpos-v1");

        let destination = BridgeProof {
            range: BridgeProofRange {
                start_height: 5,
                end_height: 5,
            },
            payload: BridgeProofPayload::SccpDestination(BridgeSccpDestinationProofV1 {
                backend: BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
                route_configuration_hash: [0x44; 32],
                encoded_artifact: vec![0x04, 0x05],
            }),
        };
        assert_eq!(destination.backend_label(), "evm-groth16-bn254-v1");

        for (payload, expected_index) in [
            (&ics.payload, 0_u32),
            (&transparent.payload, 1),
            (&native.payload, 2),
            (&destination.payload, 3),
        ] {
            let encoded = payload.encode();
            let decoded_index =
                u32::decode(&mut encoded.as_slice()).expect("bridge payload variant index decodes");
            assert_eq!(decoded_index, expected_index);
        }
    }

    #[test]
    fn bridge_proof_binding_preserves_commitment_role() {
        let manifest_hash = [0x31; 32];
        let route_hash = [0x41; 32];
        let transparent = BridgeProof {
            range: BridgeProofRange {
                start_height: 1,
                end_height: 1,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: manifest_hash,
                proof: ProofBox::new("halo2/mock".into(), vec![1]),
                recursion_depth: None,
            }),
        };
        let native = BridgeProof {
            range: BridgeProofRange {
                start_height: 2,
                end_height: 2,
            },
            payload: BridgeProofPayload::NativeProtocol(BridgeNativeProtocolProofV1 {
                backend: BridgeNativeProofBackendV1::EthereumBeacon,
                route_configuration_hash: route_hash,
                encoded_envelope: vec![2],
            }),
        };

        assert_eq!(
            transparent.binding(),
            BridgeProofBinding::VerifierManifest(manifest_hash)
        );
        assert_eq!(
            native.binding(),
            BridgeProofBinding::SccpRouteConfigurationV1(route_hash)
        );
        assert!(transparent.binding().is_well_formed());
        assert!(native.binding().is_well_formed());
        assert_ne!(
            BridgeProofBinding::VerifierManifest(route_hash),
            BridgeProofBinding::SccpRouteConfigurationV1(route_hash),
            "equal bytes in different commitment roles must remain distinguishable"
        );

        let mut bit_flipped = route_hash;
        bit_flipped[17] ^= 0x80;
        assert_ne!(
            native.binding(),
            BridgeProofBinding::SccpRouteConfigurationV1(bit_flipped)
        );
        assert!(!BridgeProofBinding::VerifierManifest([0; 32]).is_well_formed());
        assert!(!BridgeProofBinding::SccpRouteConfigurationV1([0; 32]).is_well_formed());
    }

    #[test]
    fn native_bridge_proof_backend_is_closed_and_roundtrips() {
        let backends = [
            (
                BridgeNativeProofBackendV1::EthereumBeacon,
                "bridge/sccp/native/ethereum-beacon-v1",
            ),
            (
                BridgeNativeProofBackendV1::BscParlia,
                "bridge/sccp/native/bsc-parlia-v1",
            ),
            (
                BridgeNativeProofBackendV1::TronDpos,
                "bridge/sccp/native/tron-dpos-v1",
            ),
        ];
        for &(backend, expected_label) in &backends {
            assert_eq!(backend.backend_label(), expected_label);
            let encoded = backend.encode();
            let decoded = BridgeNativeProofBackendV1::decode_all(&mut &encoded[..])
                .expect("native backend must roundtrip");
            assert_eq!(decoded, backend);
        }
        let external_profiles = [
            SccpNetworkV1::EthereumMainnet,
            SccpNetworkV1::EthereumSepolia,
            SccpNetworkV1::BscMainnet,
            SccpNetworkV1::BscTestnet,
            SccpNetworkV1::TronMainnet,
            SccpNetworkV1::TronNile,
            SccpNetworkV1::TronShasta,
        ];
        for &(backend, _) in &backends {
            for source in external_profiles {
                let expected = matches!(
                    (backend, source),
                    (
                        BridgeNativeProofBackendV1::EthereumBeacon,
                        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::EthereumSepolia
                    ) | (
                        BridgeNativeProofBackendV1::BscParlia,
                        SccpNetworkV1::BscMainnet | SccpNetworkV1::BscTestnet
                    ) | (
                        BridgeNativeProofBackendV1::TronDpos,
                        SccpNetworkV1::TronMainnet
                            | SccpNetworkV1::TronNile
                            | SccpNetworkV1::TronShasta
                    )
                );
                assert_eq!(backend.supports_source_network(source), expected);
            }
            assert!(!backend.supports_source_network(SccpNetworkV1::SoraTaira));
        }
        assert!(BridgeNativeProofBackendV1::decode_all(&mut &[0xff][..]).is_err());

        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 7,
                end_height: 7,
            },
            payload: BridgeProofPayload::NativeProtocol(BridgeNativeProtocolProofV1 {
                backend: BridgeNativeProofBackendV1::TronDpos,
                route_configuration_hash: [0x44; 32],
                encoded_envelope: vec![0xaa, 0xbb, 0xcc],
            }),
        };
        let encoded = proof.encode();
        let decoded = BridgeProof::decode_all(&mut &encoded[..]).expect("native proof decodes");
        assert_eq!(decoded, proof);
        assert_eq!(decoded.backend_label(), "bridge/sccp/native/tron-dpos-v1");
        let BridgeProofPayload::NativeProtocol(native) = &decoded.payload else {
            panic!("decoded native payload changed variant")
        };
        assert!(native.is_well_formed());
        assert!(
            !BridgeNativeProtocolProofV1 {
                route_configuration_hash: [0; 32],
                ..native.clone()
            }
            .is_well_formed()
        );
        assert!(
            !BridgeNativeProtocolProofV1 {
                encoded_envelope: Vec::new(),
                ..native.clone()
            }
            .is_well_formed()
        );

        let zero_anchor = SccpNativeTrustAnchorV1 {
            backend: BridgeNativeProofBackendV1::EthereumBeacon,
            anchor_hash: [0; 32],
            checkpoint_height: 1,
        };
        assert!(!zero_anchor.is_well_formed());
        let anchor = SccpNativeTrustAnchorV1 {
            backend: BridgeNativeProofBackendV1::EthereumBeacon,
            anchor_hash: [0x91; 32],
            checkpoint_height: 1,
        };
        assert!(anchor.is_well_formed());
        let encoded = anchor.encode();
        let decoded = SccpNativeTrustAnchorV1::decode_all(&mut &encoded[..])
            .expect("native trust anchor must roundtrip");
        assert_eq!(decoded, anchor);

        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&anchor).expect("native trust anchor JSON encodes");
            let decoded = norito::json::from_str::<SccpNativeTrustAnchorV1>(&json)
                .expect("native trust anchor JSON decodes");
            assert_eq!(decoded, anchor);
            let unknown_backend = json.replace("ethereum_beacon_v1", "unknown_native_v1");
            assert_ne!(unknown_backend, json);
            assert!(norito::json::from_str::<SccpNativeTrustAnchorV1>(&unknown_backend).is_err());
        }
    }

    #[test]
    fn sccp_destination_container_separates_all_commitment_roles() {
        let proof = BridgeSccpDestinationProofV1 {
            backend: BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
            route_configuration_hash: [0x71; 32],
            encoded_artifact: vec![1, 2, 3],
        };
        assert!(proof.is_well_formed_for([0x72; 32], [0x73; 32]));
        assert!(!proof.is_well_formed_for([0x71; 32], [0x73; 32]));
        assert!(!proof.is_well_formed_for([0x72; 32], [0x71; 32]));
        assert!(!proof.is_well_formed_for([0x72; 32], [0x72; 32]));
        assert!(
            !BridgeSccpDestinationProofV1 {
                route_configuration_hash: [0; 32],
                ..proof.clone()
            }
            .is_well_formed_for([0x72; 32], [0x73; 32])
        );
        assert!(
            !BridgeSccpDestinationProofV1 {
                encoded_artifact: Vec::new(),
                ..proof
            }
            .is_well_formed_for([0x72; 32], [0x73; 32])
        );
    }

    #[test]
    fn signer_indices_from_bitmap_collects_sparse_signers() {
        let indices =
            signer_indices_from_bitmap(&[0b0010_1001], 6).expect("sparse bitmap should decode");
        assert_eq!(indices, vec![0, 3, 5]);
    }

    #[test]
    fn bridge_finality_min_signatures_matches_quorum_policy() {
        assert_eq!(BridgeFinalityVerifier::min_signatures(0), 0);
        assert_eq!(BridgeFinalityVerifier::min_signatures(1), 1);
        assert_eq!(BridgeFinalityVerifier::min_signatures(3), 3);
        assert_eq!(BridgeFinalityVerifier::min_signatures(4), 3);
        assert_eq!(BridgeFinalityVerifier::min_signatures(6), 5);
        assert_eq!(BridgeFinalityVerifier::min_signatures(7), 5);
    }

    #[test]
    fn bridge_finality_rejects_pop_length_mismatch() {
        let keys = vec![checked_bls_keypair(), checked_bls_keypair()];
        let proof = make_finality_proof("bridge-pop-mismatch", 1, 0, &keys);
        let mut verifier =
            BridgeFinalityVerifier::new("bridge-pop-mismatch".parse().expect("chain id"));
        let mut bad = proof.clone();
        bad.validator_set_pops.pop();

        let err = verifier
            .verify(&bad)
            .expect_err("validator pop length mismatch should fail");
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::ValidatorSetPopLengthMismatch {
                expected: 2,
                got: 1
            }
        ));
    }

    #[test]
    fn wrapped_asset_roundtrip() {
        let def = WrappedAssetDef {
            origin_chain: b"btc".to_vec(),
            origin_asset_id: b"btc:mainnet".to_vec(),
            bridge_id: b"btc->iroha".to_vec(),
        };
        let buf = def.encode();
        let dec = WrappedAssetDef::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(def, dec);
    }

    #[test]
    fn receipt_roundtrip() {
        let r = BridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: Some([0x22; 32]),
            proof_hash: [0x33; 32],
            amount: 42,
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        };
        let buf = r.encode();
        let dec = BridgeReceipt::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(r, dec);
    }

    #[test]
    fn sccp_outbound_message_key_roundtrip() {
        let key = SccpOutboundMessageKeyV1::new(
            SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::BscTestnet,
            },
            [0x42; 32],
        )
        .expect("valid outbound replay key");
        let buf = key.encode();
        let dec = SccpOutboundMessageKeyV1::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(key, dec);
    }

    #[test]
    fn sccp_outbound_message_record_roundtrip() {
        let record = SccpOutboundMessageRecordV1 {
            destination_binding_hash: [0x23; 32],
            route_configuration_hash: [0x25; 32],
            payload_hash: [0x24; 32],
            recorded_at_height: 77,
        };
        let buf = record.encode();
        let dec = SccpOutboundMessageRecordV1::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(record, dec);
    }

    #[test]
    fn bridge_proof_roundtrip() {
        let leaves = vec![[0xAA; 32], [0xBB; 32]];
        let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
        let root_bytes: [u8; 32] = *tree.root().expect("root").as_ref();
        let proof = tree.get_proof(0).expect("proof");

        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 1,
                end_height: 2,
            },
            payload: BridgeProofPayload::Ics(BridgeIcsProof {
                verifier_manifest_hash: [0x55; 32],
                state_root: root_bytes,
                leaf_hash: leaves[0],
                proof,
                hash_function: BridgeHashFunction::Sha256,
            }),
        };
        let buf = proof.encode();
        let dec = BridgeProof::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(proof, dec);
    }

    #[cfg(feature = "json")]
    #[test]
    fn bridge_proof_json_rejects_unknown_fields_at_every_typed_boundary() {
        let leaves = vec![[0xAA; 32], [0xBB; 32]];
        let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 1,
                end_height: 1,
            },
            payload: BridgeProofPayload::Ics(BridgeIcsProof {
                verifier_manifest_hash: [0x55; 32],
                state_root: *tree.root().expect("root").as_ref(),
                leaf_hash: leaves[0],
                proof: tree.get_proof(0).expect("proof"),
                hash_function: BridgeHashFunction::Sha256,
            }),
        };
        let canonical = norito::json::to_json(&proof).expect("serialize bridge proof JSON");
        assert_eq!(
            norito::json::from_json::<BridgeProof>(&canonical).expect("canonical JSON decodes"),
            proof
        );

        let mut retired_pin_value =
            norito::json::to_value(&proof).expect("serialize bridge proof value");
        let norito::json::Value::Object(retired_pin_object) = &mut retired_pin_value else {
            panic!("bridge proof JSON must be an object")
        };
        retired_pin_object.insert("pinned".into(), norito::json::Value::Bool(true));
        let retired_pin =
            norito::json::to_json(&retired_pin_value).expect("serialize retired pin field");
        assert!(
            norito::json::from_json::<BridgeProof>(&retired_pin).is_err(),
            "retired caller-controlled retention hint must fail closed"
        );

        for path in [
            Vec::<&str>::new(),
            vec!["range"],
            vec!["payload"],
            vec!["payload", "payload"],
            vec!["payload", "payload", "hash_function"],
        ] {
            let mut hostile = norito::json::to_value(&proof).expect("serialize bridge proof value");
            let mut current = &mut hostile;
            for field in &path {
                let norito::json::Value::Object(object) = current else {
                    panic!("bridge proof JSON path component `{field}` is not an object")
                };
                current = object
                    .get_mut(*field)
                    .unwrap_or_else(|| panic!("bridge proof JSON path component `{field}` absent"));
            }
            let norito::json::Value::Object(object) = current else {
                panic!("bridge proof JSON target at {path:?} is not an object")
            };
            object.insert("adversarial_extension".into(), norito::json::Value::Null);
            let hostile_json =
                norito::json::to_json(&hostile).expect("serialize hostile bridge proof JSON");
            assert!(
                norito::json::from_json::<BridgeProof>(&hostile_json).is_err(),
                "unknown field at {path:?} must reject"
            );
        }

        let duplicate = canonical.replacen("\"range\":", "\"range\":null,\"range\":", 1);
        assert_ne!(duplicate, canonical);
        assert!(norito::json::from_json::<BridgeProof>(&duplicate).is_err());
    }

    #[test]
    fn bridge_proof_decoder_rejects_legacy_truncated_and_trailing_encodings() {
        #[derive(Encode)]
        struct LegacyBridgeProof {
            range: BridgeProofRange,
            manifest_hash: [u8; 32],
            payload: BridgeProofPayload,
            pinned: bool,
        }

        #[derive(Encode)]
        struct CallerPinnedBridgeProof {
            range: BridgeProofRange,
            payload: BridgeProofPayload,
            pinned: bool,
        }

        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 5,
                end_height: 6,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: [0x51; 32],
                proof: ProofBox::new("stark/mock".into(), vec![7, 8, 9]),
                recursion_depth: Some(3),
            }),
        };
        let canonical = proof.encode();
        assert_eq!(
            BridgeProof::decode_all(&mut canonical.as_slice())
                .expect("canonical bridge proof decodes"),
            proof.clone()
        );

        for end in 0..canonical.len() {
            let mut truncated: &[u8] = &canonical[..end];
            assert!(
                BridgeProof::decode_all(&mut truncated).is_err(),
                "truncated bridge proof unexpectedly decoded at byte {end}"
            );
        }
        let mut trailing = canonical;
        trailing.push(0);
        assert!(BridgeProof::decode_all(&mut trailing.as_slice()).is_err());

        let legacy = LegacyBridgeProof {
            range: proof.range,
            manifest_hash: [0xff; 32],
            payload: proof.payload.clone(),
            pinned: true,
        }
        .encode();
        assert!(BridgeProof::decode_all(&mut legacy.as_slice()).is_err());

        let caller_pinned = CallerPinnedBridgeProof {
            range: proof.range,
            payload: proof.payload,
            pinned: true,
        }
        .encode();
        assert!(
            BridgeProof::decode_all(&mut caller_pinned.as_slice()).is_err(),
            "retired caller-controlled retention field must fail binary decoding"
        );
    }

    #[test]
    fn bridge_proof_transparent_zk_roundtrip() {
        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 8,
                end_height: 8,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: [0x61; 32],
                proof: ProofBox::new("stark/mock".into(), vec![9, 8, 7, 6]),
                recursion_depth: Some(1),
            }),
        };
        let buf = proof.encode();
        let dec = BridgeProof::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(proof, dec);
    }

    #[test]
    fn bridge_proof_record_roundtrip() {
        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 3,
                end_height: 4,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: [0x71; 32],
                proof: ProofBox::new("groth16/mock".into(), vec![1, 2, 3, 4]),
                recursion_depth: None,
            }),
        };
        let record = BridgeProofRecord {
            proof,
            commitment: [0x81; 32],
            size_bytes: 4096,
        };
        let buf = record.encode();
        let dec = BridgeProofRecord::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(record, dec);
    }

    #[test]
    fn bridge_finality_proof_roundtrip() {
        let keys = vec![checked_bls_keypair()];
        let proof = make_finality_proof("proof-chain", 1, 0, &keys);
        let buf = proof.encode();
        let dec = BridgeFinalityProof::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(proof, dec);
    }

    #[test]
    fn bridge_finality_proof_roundtrip_without_validator_pops() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("proof-chain-no-pops", 2, 0, &keys);
        proof.validator_set_pops.clear();

        let buf = proof.encode();
        let dec = BridgeFinalityProof::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(proof, dec);
        assert!(dec.validator_set_pops.is_empty());
    }

    #[test]
    fn bridge_finality_bundle_roundtrip() {
        let validator_keys: Vec<_> = (0..2).map(|_| checked_bls_keypair()).collect();
        let next_keys: Vec<_> = (0..2).map(|_| checked_bls_keypair()).collect();
        let signature_key = checked_random_keypair();
        let proof = make_finality_proof("bundle-chain", 4, 2, &validator_keys);
        let authority_set = BridgeAuthoritySet {
            id: 9,
            validator_set: proof.commit_qc.validator_set.clone(),
            validator_set_hash: proof.commit_qc.validator_set_hash,
            validator_set_hash_version: proof.commit_qc.validator_set_hash_version,
        };
        let next_authority_set = BridgeAuthoritySet {
            id: 10,
            validator_set: validator_set_from_keys(&next_keys),
            validator_set_hash: HashOf::new(&validator_set_from_keys(&next_keys)),
            validator_set_hash_version: crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        };
        let bundle = BridgeFinalityBundle {
            commitment: BridgeCommitment {
                chain_id: proof.chain_id.clone(),
                authority_set: authority_set.clone(),
                block_height: proof.height,
                block_hash: proof.block_hash,
                mmr_root: Some([0x91; 32]),
                mmr_leaf_index: Some(3),
                mmr_peaks: Some(vec![[0x92; 32], [0x93; 32]]),
                next_authority_set: Some(next_authority_set),
            },
            justification: BridgeCommitmentJustification {
                signatures: vec![make_block_signature(0, &signature_key, &proof.block_header)],
            },
            block_header: proof.block_header,
            commit_qc: proof.commit_qc,
        };

        let buf = bundle.encode();
        let dec = BridgeFinalityBundle::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(bundle, dec);
        assert_eq!(dec.commitment.authority_set, authority_set);
    }

    #[test]
    fn bridge_commitment_roundtrip_without_optional_fields() {
        let keys: Vec<_> = (0..2).map(|_| checked_bls_keypair()).collect();
        let authority_set = BridgeAuthoritySet {
            id: 12,
            validator_set: validator_set_from_keys(&keys),
            validator_set_hash: HashOf::new(&validator_set_from_keys(&keys)),
            validator_set_hash_version: crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        };
        let header = crate::block::BlockHeader::new(
            NonZeroU64::new(12).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let commitment = BridgeCommitment {
            chain_id: "commitment-chain".parse().expect("chain id"),
            authority_set,
            block_height: 12,
            block_hash: header.hash(),
            mmr_root: None,
            mmr_leaf_index: None,
            mmr_peaks: None,
            next_authority_set: None,
        };

        let buf = commitment.encode();
        let dec = BridgeCommitment::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(commitment, dec);
    }

    #[test]
    fn consensus_domain_changes_with_chain_tag_and_mode() {
        let chain_a: ChainId = "chain-a".parse().expect("chain id");
        let chain_b: ChainId = "chain-b".parse().expect("chain id");
        let vote_v2 = consensus_domain(
            &chain_a,
            "Vote",
            b"v2",
            crate::block::consensus::PERMISSIONED_TAG,
        );

        assert_eq!(
            vote_v2,
            consensus_domain(
                &chain_a,
                "Vote",
                b"v2",
                crate::block::consensus::PERMISSIONED_TAG,
            )
        );
        assert_ne!(
            vote_v2,
            consensus_domain(
                &chain_b,
                "Vote",
                b"v2",
                crate::block::consensus::PERMISSIONED_TAG,
            )
        );
        assert_ne!(
            vote_v2,
            consensus_domain(
                &chain_a,
                "Proposal",
                b"v2",
                crate::block::consensus::PERMISSIONED_TAG,
            )
        );
        assert_ne!(
            vote_v2,
            consensus_domain(
                &chain_a,
                "Vote",
                b"v1",
                crate::block::consensus::PERMISSIONED_TAG,
            )
        );
        assert_ne!(
            vote_v2,
            consensus_domain(&chain_a, "Vote", b"v2", crate::block::consensus::NPOS_TAG)
        );
    }

    #[test]
    fn commit_vote_preimage_serializes_vote_fields_in_order() {
        let keys = vec![checked_bls_keypair()];
        let proof = make_finality_proof("preimage-chain", 7, 3, &keys);
        let preimage = commit_vote_preimage(&proof.chain_id, &proof.commit_qc);
        let domain = consensus_domain(&proof.chain_id, "Vote", b"v2", &proof.commit_qc.mode_tag);

        let mut offset = 0usize;
        assert_eq!(&preimage[offset..offset + 32], &domain);
        offset += 32;
        assert_eq!(
            &preimage[offset..offset + Hash::LENGTH],
            proof.commit_qc.subject_block_hash.as_ref().as_ref()
        );
        offset += Hash::LENGTH;
        assert_eq!(
            &preimage[offset..offset + Hash::LENGTH],
            proof.commit_qc.parent_state_root.as_ref()
        );
        offset += Hash::LENGTH;
        assert_eq!(
            &preimage[offset..offset + Hash::LENGTH],
            proof.commit_qc.post_state_root.as_ref()
        );
        offset += Hash::LENGTH;
        assert_eq!(
            &preimage[offset..offset + 8],
            &proof.commit_qc.height.to_be_bytes()
        );
        offset += 8;
        assert_eq!(
            &preimage[offset..offset + 8],
            &proof.commit_qc.view.to_be_bytes()
        );
        offset += 8;
        assert_eq!(
            &preimage[offset..offset + 8],
            &proof.commit_qc.epoch.to_be_bytes()
        );
        offset += 8;
        assert_eq!(
            &preimage[offset..offset + Hash::LENGTH],
            proof.commit_qc.chain_order_hash.as_ref()
        );
        offset += Hash::LENGTH;
        assert_eq!(
            &preimage[offset..offset + 8],
            &proof.commit_qc.rechain_seq.to_be_bytes()
        );
        offset += 8;
        assert_eq!(preimage[offset], proof.commit_qc.phase as u8);
        offset += 1;
        assert_eq!(preimage[offset], 0);
        offset += 1;
        assert_eq!(offset, preimage.len());
    }

    #[test]
    fn commit_vote_preimage_serializes_highest_qc_when_present() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("preimage-highest-qc-chain", 7, 3, &keys);
        let highest_qc = crate::consensus::QcRef {
            height: 6,
            view: 2,
            epoch: 3,
            subject_block_hash: proof.block_hash,
            phase: crate::block::consensus::CertPhase::Prepare,
        };
        proof.commit_qc.highest_qc = Some(highest_qc);

        let preimage = commit_vote_preimage(&proof.chain_id, &proof.commit_qc);
        let mut offset = 32 + Hash::LENGTH * 3 + 8 * 3 + Hash::LENGTH + 8 + 1;

        assert_eq!(preimage[offset], 1);
        offset += 1;
        assert_eq!(
            &preimage[offset..offset + 8],
            &highest_qc.height.to_be_bytes()
        );
        offset += 8;
        assert_eq!(
            &preimage[offset..offset + 8],
            &highest_qc.view.to_be_bytes()
        );
        offset += 8;
        assert_eq!(
            &preimage[offset..offset + 8],
            &highest_qc.epoch.to_be_bytes()
        );
        offset += 8;
        assert_eq!(
            &preimage[offset..offset + Hash::LENGTH],
            highest_qc.subject_block_hash.as_ref().as_ref()
        );
        offset += Hash::LENGTH;
        assert_eq!(preimage[offset], highest_qc.phase as u8);
        offset += 1;
        assert_eq!(offset, preimage.len());
    }

    #[test]
    fn signer_indices_from_bitmap_accepts_empty_roster() {
        let indices = signer_indices_from_bitmap(&[], 0).expect("empty roster should decode");
        assert!(indices.is_empty());
    }

    #[test]
    fn signer_indices_from_bitmap_collects_bits_across_multiple_bytes() {
        let indices = signer_indices_from_bitmap(&[0b0000_0000, 0b0000_0101], 11).expect("bitmap");
        assert_eq!(indices, vec![8, 10]);
    }

    #[test]
    fn verifier_with_validator_set_accepts_after_epoch_anchor_update() {
        let keys: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let proof = make_finality_proof("chain-a", 1, 0, &keys);
        let mut verifier = BridgeFinalityVerifier::with_validator_set(
            proof.chain_id.clone(),
            proof.commit_qc.validator_set_hash,
            proof.commit_qc.validator_set_hash_version,
        );

        let err = verifier
            .verify(&proof)
            .expect_err("epoch anchor should still be required");
        assert!(matches!(err, BridgeFinalityVerifyError::MissingEpochAnchor));

        verifier.set_epoch_anchor(proof.commit_qc.epoch);
        verifier
            .verify(&proof)
            .expect("validator-set-only constructor should accept once epoch anchor is set");
    }

    #[test]
    fn verifier_rejects_wrong_chain_id() {
        let keys: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let proof = make_finality_proof("chain-a", 1, 0, &keys);
        let mut verifier = BridgeFinalityVerifier::new("chain-b".parse().expect("chain id parses"));

        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::ChainIdMismatch { .. }
        ));
    }

    #[test]
    fn verifier_accepts_consensus_hash_when_result_merkle_root_is_present() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.block_header = crate::block::BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            Some(HashOf::from_untyped_unchecked(Hash::prehashed(
                [0xAB; Hash::LENGTH],
            ))),
            0,
            0,
        );
        proof.block_hash = proof.block_header.hash();
        proof.commit_qc.subject_block_hash = proof.block_hash;
        let preimage = commit_vote_preimage(&proof.chain_id, &proof.commit_qc);
        let signatures: Vec<Vec<u8>> = keys
            .iter()
            .map(|kp| checked_commit_vote_signature_payload(kp, &preimage))
            .collect();
        let signature_refs: Vec<&[u8]> = signatures.iter().map(Vec::as_slice).collect();
        proof.commit_qc.aggregate.bls_aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                .expect("aggregate signatures");

        let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
            proof.chain_id.clone(),
            proof.commit_qc.validator_set_hash,
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            0,
        );

        verifier
            .verify(&proof)
            .expect("consensus hash should ignore result merkle root");
    }

    #[test]
    fn verifier_accepts_exact_quorum_signer_subset() {
        let keys: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let proof = make_finality_proof_with_signers_and_mode(
            "chain-a",
            1,
            0,
            &keys,
            &[0, 2, 3],
            crate::block::consensus::PERMISSIONED_TAG,
        );
        let mut verifier = verifier_anchored_to(&proof);

        verifier
            .verify(&proof)
            .expect("exact quorum subset should verify");
    }

    #[test]
    fn verifier_accepts_npos_mode_with_matching_signature_domain() {
        let keys = vec![checked_bls_keypair()];
        let proof = make_finality_proof_with_signers_and_mode(
            "chain-a",
            1,
            0,
            &keys,
            &[0],
            crate::block::consensus::NPOS_TAG,
        );
        let mut verifier = verifier_anchored_to(&proof);

        verifier
            .verify(&proof)
            .expect("alternate consensus mode tag should verify when signature domain matches");
    }

    #[test]
    fn validate_commit_qc_rejects_empty_roster_without_aggregate_signature() {
        let header = crate::block::BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let certificate = crate::consensus::Qc {
            phase: crate::block::consensus::CertPhase::Commit,
            subject_block_hash: header.hash(),
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height: 1,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: crate::block::consensus::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&Vec::<PeerId>::new()),
            validator_set_hash_version: crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: Vec::new(),
            aggregate: crate::consensus::QcAggregate {
                signers_bitmap: Vec::new(),
                bls_aggregate_signature: Vec::new(),
            },
        };

        let err = BridgeFinalityVerifier::validate_commit_qc(
            &"chain-a".parse().expect("chain id"),
            &certificate,
            &[],
        )
        .expect_err("helper should still require an aggregate signature");
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::AggregateSignatureMissing
        ));
    }

    #[test]
    fn verifier_does_not_advance_height_after_failed_proof() {
        let keys: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let expected_hash = HashOf::new(&validator_set_from_keys(&keys));
        let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
            "chain-a".parse().expect("chain id parses"),
            expected_hash,
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            0,
        );

        let first = make_finality_proof("chain-a", 1, 0, &keys);
        verifier.verify(&first).expect("first proof accepted");

        let bad_second = make_finality_proof("chain-a", 2, 1, &keys);
        let err = verifier.verify(&bad_second).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnexpectedEpoch {
                expected: 0,
                got: 1
            }
        ));

        let good_second = make_finality_proof("chain-a", 2, 0, &keys);
        verifier
            .verify(&good_second)
            .expect("failed proof must not advance verifier state");

        let stale = verifier.verify(&good_second).unwrap_err();
        assert!(matches!(
            stale,
            BridgeFinalityVerifyError::StaleHeight {
                latest: 2,
                height: 2
            }
        ));
    }

    #[test]
    fn verifier_rejects_certificate_height_mismatch() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.commit_qc.height = 2;

        let mut verifier = BridgeFinalityVerifier::new(proof.chain_id.clone());
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::CertificateHeightMismatch {
                proof_height: 1,
                certificate_height: 2
            }
        ));
    }

    #[test]
    fn verifier_rejects_non_commit_certificate_phase() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.commit_qc.phase = crate::block::consensus::CertPhase::Prepare;

        let mut verifier = BridgeFinalityVerifier::new(proof.chain_id.clone());
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::CertificatePhaseMismatch {
                expected: crate::block::consensus::CertPhase::Commit,
                got: crate::block::consensus::CertPhase::Prepare
            }
        ));
    }

    #[test]
    fn verifier_rejects_mismatched_block_hash_field() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.block_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xCD; Hash::LENGTH]));

        let mut verifier = BridgeFinalityVerifier::new(proof.chain_id.clone());
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::BlockHashMismatch { .. }
        ));
    }

    #[test]
    fn verifier_rejects_mismatched_certificate_hash_field() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.commit_qc.subject_block_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0xCE; Hash::LENGTH]));

        let mut verifier = BridgeFinalityVerifier::new(proof.chain_id.clone());
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::BlockHashMismatch { .. }
        ));
    }

    #[test]
    fn verifier_rejects_unsupported_validator_set_hash_version() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        let unsupported_version = crate::consensus::VALIDATOR_SET_HASH_VERSION_V1 + 1;
        proof.commit_qc.validator_set_hash_version = unsupported_version;

        let mut verifier = BridgeFinalityVerifier::new(proof.chain_id.clone());
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnsupportedValidatorSetHashVersion {
                version
            } if version == unsupported_version
        ));
    }

    #[test]
    fn verifier_rejects_empty_validator_set() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.commit_qc.validator_set.clear();
        proof.commit_qc.validator_set_hash = HashOf::new(&proof.commit_qc.validator_set);
        proof.commit_qc.aggregate.signers_bitmap.clear();
        proof.validator_set_pops.clear();

        let mut verifier = verifier_anchored_to(&proof);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(err, BridgeFinalityVerifyError::EmptyValidatorSet));
    }

    #[test]
    fn verifier_rejects_signer_bitmap_length_mismatch() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.commit_qc.aggregate.signers_bitmap.clear();

        let mut verifier = verifier_anchored_to(&proof);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::SignerBitmapLengthMismatch {
                expected: 1,
                got: 0
            }
        ));
    }

    #[test]
    fn verifier_rejects_signer_bitmap_index_out_of_range() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.commit_qc.aggregate.signers_bitmap = vec![0b0000_0011];

        let mut verifier = verifier_anchored_to(&proof);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::SignatureIndexOutOfRange { index: 1, len: 1 }
        ));
    }

    #[test]
    fn verifier_rejects_insufficient_signers_for_quorum() {
        let keys: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.commit_qc.aggregate.signers_bitmap = vec![0b0000_0001];

        let mut verifier = verifier_anchored_to(&proof);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::InsufficientSigners {
                required: 3,
                collected: 1
            }
        ));
    }

    #[test]
    fn verifier_rejects_missing_aggregate_signature() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.commit_qc.aggregate.bls_aggregate_signature.clear();

        let mut verifier = verifier_anchored_to(&proof);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::AggregateSignatureMissing
        ));
    }

    #[test]
    fn verifier_rejects_non_bls_validator_key_in_commit_qc() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        let wrong_key = checked_random_keypair_with_algorithm(Algorithm::Ed25519);
        assert_eq!(
            wrong_key
                .public_key()
                .try_algorithm()
                .expect("checked public-key algorithm"),
            Algorithm::Ed25519
        );
        proof.commit_qc.validator_set = vec![PeerId::from(wrong_key.public_key().clone())];
        proof.commit_qc.validator_set_hash = HashOf::new(&proof.commit_qc.validator_set);

        let mut verifier = verifier_anchored_to(&proof);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::InvalidValidatorKeyAlgorithm {
                index: 0,
                algorithm: Algorithm::Ed25519
            }
        ));
    }

    #[test]
    fn verifier_rejects_invalid_aggregate_signature_payload() {
        let keys = vec![checked_bls_keypair()];
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof
            .commit_qc
            .aggregate
            .bls_aggregate_signature
            .pop()
            .expect("aggregate signature should not be empty before truncation");

        let mut verifier = verifier_anchored_to(&proof);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::InvalidAggregateSignature
        ));
    }

    #[test]
    fn verifier_rejects_stale_and_advanced_heights() {
        let keys: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
            "chain-a".parse().expect("chain id parses"),
            HashOf::new(&validator_set_from_keys(&keys)),
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            0,
        );

        let first = make_finality_proof("chain-a", 1, 0, &keys);
        verifier.verify(&first).expect("first proof accepted");

        let stale_err = verifier.verify(&first).unwrap_err();
        assert!(matches!(
            stale_err,
            BridgeFinalityVerifyError::StaleHeight {
                latest: 1,
                height: 1
            }
        ));

        let advanced = make_finality_proof("chain-a", 3, 0, &keys);
        let advanced_err = verifier.verify(&advanced).unwrap_err();
        assert!(matches!(
            advanced_err,
            BridgeFinalityVerifyError::AdvancedHeight {
                latest: 1,
                height: 3
            }
        ));
    }

    #[test]
    fn verifier_rejects_replayed_validator_set_after_anchor() {
        let old_keys: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let new_keys: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let expected_hash = HashOf::new(&validator_set_from_keys(&new_keys));
        let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
            "chain-a".parse().expect("chain id parses"),
            expected_hash,
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            0,
        );

        let proof = make_finality_proof("chain-a", 1, 0, &old_keys);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnexpectedValidatorSet { .. }
        ));
    }

    #[test]
    fn verifier_rejects_unexpected_epoch_anchor() {
        let keys: Vec<_> = (0..3).map(|_| checked_bls_keypair()).collect();
        let expected_hash = HashOf::new(&validator_set_from_keys(&keys));
        let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
            "chain-a".parse().expect("chain id parses"),
            expected_hash,
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            5,
        );

        let proof = make_finality_proof("chain-a", 1, 4, &keys);
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnexpectedEpoch {
                expected: 5,
                got: 4
            }
        ));
    }

    #[test]
    fn verifier_rejects_tampered_validator_set_hash() {
        let keys: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let mut proof = make_finality_proof("chain-a", 1, 0, &keys);
        proof.commit_qc.validator_set_hash = HashOf::new(&Vec::<PeerId>::new());

        let mut verifier = BridgeFinalityVerifier::new("chain-a".parse().expect("chain id parses"));
        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::ValidatorSetHashMismatch { .. }
        ));
    }

    #[test]
    fn verifier_rejects_prior_epoch_after_anchor_rotation() {
        let epoch0_keys: Vec<_> = (0..3).map(|_| checked_bls_keypair()).collect();
        let epoch1_keys: Vec<_> = (0..3).map(|_| checked_bls_keypair()).collect();
        let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
            "chain-a".parse().expect("chain id parses"),
            HashOf::new(&validator_set_from_keys(&epoch0_keys)),
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            0,
        );

        let proof_epoch0 = make_finality_proof("chain-a", 1, 0, &epoch0_keys);
        verifier
            .verify(&proof_epoch0)
            .expect("initial anchored proof should verify");

        verifier.set_validator_set_and_epoch_anchor(
            HashOf::new(&validator_set_from_keys(&epoch1_keys)),
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            1,
        );

        let replayed = make_finality_proof("chain-a", 2, 0, &epoch0_keys);
        let err = verifier.verify(&replayed).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnexpectedEpoch {
                expected: 1,
                got: 0
            }
        ));
    }

    #[test]
    fn verifier_accepts_epoch_and_roster_rotation_after_combined_anchor_update() {
        let roster_a: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let roster_b: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
            "chain-a".parse().expect("chain id parses"),
            HashOf::new(&validator_set_from_keys(&roster_a)),
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            0,
        );

        let proof_a = make_finality_proof("chain-a", 1, 0, &roster_a);
        verifier.verify(&proof_a).expect("first proof accepted");

        let proof_b = make_finality_proof("chain-a", 2, 1, &roster_b);
        let err = verifier.verify(&proof_b).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnexpectedEpoch {
                expected: 0,
                got: 1
            }
        ));

        verifier.set_validator_set_and_epoch_anchor(
            HashOf::new(&validator_set_from_keys(&roster_b)),
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            1,
        );

        verifier
            .verify(&proof_b)
            .expect("combined anchor rotation should accept the new roster and epoch");
    }

    #[test]
    fn verifier_accepts_roster_change_after_anchor_update() {
        let roster_a: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let roster_b: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let mut verifier = BridgeFinalityVerifier::with_validator_set_and_epoch(
            "chain-a".parse().expect("chain id parses"),
            HashOf::new(&validator_set_from_keys(&roster_a)),
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            0,
        );

        let proof_a = make_finality_proof("chain-a", 1, 0, &roster_a);
        verifier.verify(&proof_a).expect("first proof accepted");

        let proof_b = make_finality_proof("chain-a", 2, 0, &roster_b);
        let err = verifier.verify(&proof_b).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnexpectedValidatorSet { .. }
        ));

        verifier.set_validator_set_anchor(
            HashOf::new(&validator_set_from_keys(&roster_b)),
            crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        );

        verifier.verify(&proof_b).expect("anchor swap accepted");
    }

    #[test]
    fn verifier_requires_explicit_epoch_and_validator_set_anchors() {
        let keys: Vec<_> = (0..4).map(|_| checked_bls_keypair()).collect();
        let proof = make_finality_proof("chain-a", 1, 0, &keys);

        let mut missing_both =
            BridgeFinalityVerifier::new("chain-a".parse().expect("chain id parses"));
        let err = missing_both
            .verify(&proof)
            .expect_err("missing anchors should fail");
        assert!(matches!(err, BridgeFinalityVerifyError::MissingEpochAnchor));

        let mut missing_validator =
            BridgeFinalityVerifier::new("chain-a".parse().expect("chain id parses"));
        missing_validator.set_epoch_anchor(0);
        let err = missing_validator
            .verify(&proof)
            .expect_err("missing validator set anchor should fail");
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::MissingValidatorSetAnchor
        ));
    }

    #[test]
    fn verifier_enforces_hash_version_updated_via_anchor_setter() {
        let keys: Vec<_> = (0..3).map(|_| checked_bls_keypair()).collect();
        let proof = make_finality_proof("chain-a", 1, 0, &keys);
        let unsupported_version = crate::consensus::VALIDATOR_SET_HASH_VERSION_V1 + 1;
        let mut verifier = BridgeFinalityVerifier::new(proof.chain_id.clone());
        verifier.set_epoch_anchor(proof.commit_qc.epoch);
        verifier.set_validator_set_anchor(proof.commit_qc.validator_set_hash, unsupported_version);

        let err = verifier.verify(&proof).unwrap_err();
        assert!(matches!(
            err,
            BridgeFinalityVerifyError::UnsupportedValidatorSetHashVersion {
                version
            } if version == crate::consensus::VALIDATOR_SET_HASH_VERSION_V1
        ));
    }
}
