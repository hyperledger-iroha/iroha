//! Bridge-related data types for wrapped assets and receipts. Feature-gated behind `bridge`.
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{NetworkId, nexus::LaneId, proof::ProofBox};
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{string::String, vec::Vec};
use thiserror::Error;
/// Versioned SCCP network, lane, and source-identity wire types.
pub mod sccp;
mod sccp_liability;
mod sccp_registry;
mod sccp_replay;
mod sccp_ton_breaker;
pub use sccp::{
    SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1, SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1,
    SCCP_TON_BASECHAIN_WORKCHAIN_V1, SCCP_TON_MAINNET_GLOBAL_ID_V1,
    SCCP_TON_MAINNET_ZERO_STATE_FILE_HASH_V1, SCCP_TON_MAINNET_ZERO_STATE_ROOT_HASH_V1,
    SCCP_TON_MASTERCHAIN_SHARD_V1, SCCP_TON_MASTERCHAIN_WORKCHAIN_V1, SCCP_TON_ZERO_STATE_SEQNO_V1,
    SccpEvmSourceEmitterV1, SccpInboundAnchorHighWaterKeyV1, SccpLaneIdV1, SccpNetworkV1,
    SccpOutboundMessageContextV1, SccpOutboundMessageDescriptorV1, SccpOutboundMessageIndexKeyV1,
    SccpOutboundMessageKeyV1, SccpOutboundPendingMessageRecordV1, SccpOutboundPendingUsageV1,
    SccpSourceEmitterV1, SccpSourceIdentityV1, SccpTonAddressV1, SccpTonSourceEmitterV1,
    SccpTronSourceEmitterV1,
};
pub use sccp_liability::SccpRouteLiabilityV1;
pub use sccp_registry::{
    SCCP_V1_MAX_GOVERNED_LANES, SCCP_V1_MAX_KEY_BYTES, SCCP_V1_MAX_LIVE_GOVERNED_ROUTES,
    SCCP_V1_MAX_LIVE_ROUTES_PER_LANE, SCCP_V1_MAX_PAYLOAD_AMOUNT_SCALE,
    SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE, SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE,
    SCCP_V1_MAX_SORA_OUTBOUND_GAS_LIMIT, SCCP_V1_SORA_OUTBOUND_EXECUTION_SEMANTICS,
    SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER, SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER,
    SCCP_V1_TAIRA_XOR_ASSET_DEFINITION_ID, SCCP_V1_TON_MAX_COINS, SCCP_V1_TON_STORAGE_VERSION,
    SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE, SccpBn254G1PointV1, SccpBn254G2PointV1,
    SccpDestinationDeploymentV1, SccpEvmDestinationDeploymentV1, SccpGovernedLaneV1,
    SccpGovernedRouteV1, SccpGroth16Bls12381IcV1, SccpGroth16Bls12381SemanticCircuitV1,
    SccpGroth16Bls12381VerifyingKeyV1, SccpGroth16Bn254IcV1, SccpGroth16Bn254SemanticCircuitV1,
    SccpGroth16Bn254VerifyingKeyV1, SccpInboundFinalityCutoffV1, SccpOutboundProofPolicyV1,
    SccpPortableVerifyingKeyRefV1, SccpRegistryV1, SccpRouteActivationV1, SccpRouteKeyV1,
    SccpRouteValidationError, SccpSemanticProofProfileV1, SccpSoraFinalityAnchorV1,
    SccpSoraOutboundExecutionPolicyV1, SccpSoraSettlementV1, SccpTonDestinationDeploymentV1,
    SccpTonMintBreakerGuardianKeysV1, SccpTronDestinationDeploymentV1,
    canonical_sccp_groth16_bls12381_public_signal_schema_bytes_v1,
    canonical_sccp_groth16_bls12381_verifying_key_bytes_v1,
    canonical_sccp_groth16_bn254_public_signal_schema_bytes_v1,
    canonical_sccp_groth16_bn254_verifying_key_bytes_v1, canonical_sccp_lane_id_bytes_v1,
    canonical_sccp_network_bytes_v1, canonical_sccp_semantic_proof_profile_bytes_v1,
    canonical_sccp_sora_finality_anchor_bytes_v1, canonical_sccp_source_emitter_bytes_v1,
    canonical_sccp_source_identity_bytes_v1, canonical_sccp_ton_raw_address_bytes_v1,
    sccp_evm_destination_binding_hash_v1, sccp_exact_evm_xor_route_config_hash_v1,
    sccp_exact_ton_xor_route_config_hash_v1, sccp_exact_tron_xor_route_config_hash_v1,
    sccp_groth16_bls12381_public_signal_schema_hash_v1,
    sccp_groth16_bls12381_verifying_key_hash_v1, sccp_groth16_bn254_public_signal_schema_hash_v1,
    sccp_groth16_bn254_verifying_key_hash_v1, sccp_lane_id_hash_v1, sccp_network_identity_hash_v1,
    sccp_network_tag_v1, sccp_route_escrow_account_id_v1, sccp_semantic_proof_profile_hash_v1,
    sccp_sora_finality_anchor_hash_v1, sccp_sora_taira_chain_id_hash_v1,
    sccp_source_emitter_identity_hash_v1, sccp_source_identity_hash_v1,
    sccp_ton_destination_binding_hash_v1, sccp_ton_groth16_bls12381_proof_profile_commitment_v1,
    sccp_tron_destination_binding_hash_v1, sccp_v1_taira_xor_asset_definition_id,
};
pub use sccp_replay::{
    SCCP_REPLAY_SMT_DEPTH_V1, SCCP_REPLAY_SMT_MAGIC_V1, SCCP_REPLAY_SMT_MAX_SIBLINGS_V1,
    SCCP_REPLAY_SMT_SHARD_COUNT_V1, SCCP_REPLAY_WITNESS_MAX_BASE64_BYTES_V1,
    SCCP_REPLAY_WITNESS_MAX_ENCODED_BYTES_V1, SccpReplayAccumulatorError,
    SccpReplayAccumulatorIdV1, SccpReplayActorV1, SccpReplayBoundaryV1, SccpReplayDeltaV1,
    SccpReplayDomainV1, SccpReplayForestV1, SccpReplayPrincipalV1, SccpReplayRecordV1,
    SccpSparseMerkleWitnessV1, SccpTonAccountV1, sccp_replay_domain_hash_v1,
    sccp_replay_empty_hashes_v1, sccp_replay_key_v1, sccp_replay_record_digest_v1,
};
pub use sccp_ton_breaker::{
    SCCP_TON_BREAKER_MAX_AGE_MS_V1, SCCP_TON_BREAKER_MAX_FUTURE_SKEW_MS_V1,
    SCCP_TON_PENDING_OPERATION_CAP_V1, SccpTonAccountStateReadbackV1, SccpTonBlockIdExtV1,
    SccpTonBreakerObservationRecordV1, SccpTonBridgePendingReadbackV1, SccpTonDeploymentReadbackV1,
    SccpTonFinalizedMasterchainBlockV1, SccpTonMasterStorageReadbackV1,
    SccpTonReplayForestReadbackV1, SccpTonRouteStorageReadbackV1, observation_is_fresh_at,
};
/// Definition metadata for a wrapped asset originating from another chain.
///
/// Stored alongside an Iroha asset definition to bind it to its origin.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
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
    /// Exact non-negative amount transferred in the asset's native precision.
    pub amount: Quantity,
    /// Canonical Iroha asset id bytes.
    pub asset_id: Vec<u8>,
    /// Recipient identifier bytes (Iroha account id or external address payload).
    pub recipient: Vec<u8>,
}
/// Hash function used by bridge Merkle proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
/// Unlike a transparent proof backend, this identifier is not a caller-chosen string. Each value
/// selects one concrete native consensus and inclusion verifier, so an unknown value fails decoding
/// instead of being routed by a node-local naming convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
    /// TRON proof using native `DPoS` replay and transaction inclusion.
    #[codec(index = 2)]
    #[norito(rename = "tron_dpos_v1")]
    TronDpos,
    /// TON proof using authenticated masterchain finality and account/message inclusion.
    #[codec(index = 3)]
    #[norito(rename = "ton_masterchain_v1")]
    TonMasterchain,
}
impl BridgeNativeProofBackendV1 {
    /// Return the stable, unambiguous bridge backend label.
    #[must_use]
    pub const fn backend_label(self) -> &'static str {
        match self {
            Self::EthereumBeacon => "bridge/sccp/native/ethereum-beacon-v1",
            Self::BscParlia => "bridge/sccp/native/bsc-parlia-v1",
            Self::TronDpos => "bridge/sccp/native/tron-dpos-v1",
            Self::TonMasterchain => "bridge/sccp/native/ton-masterchain-v1",
        }
    }
    /// Return whether V1 admits this backend for the exact source-network profile.
    ///
    /// The closed first-release inventory contains only verifier families
    /// with exact value-moving implementations.
    #[must_use]
    pub const fn supports_source_network(self, source: SccpNetworkV1) -> bool {
        matches!(
            (self, source),
            (Self::EthereumBeacon, SccpNetworkV1::EthereumMainnet)
                | (Self::BscParlia, SccpNetworkV1::BscMainnet)
                | (Self::TronDpos, SccpNetworkV1::TronMainnet)
                | (Self::TonMasterchain, SccpNetworkV1::TonMainnet)
        )
    }
}
/// Governed protocol-native trust anchor for one SCCP lane.
///
/// `anchor_hash` is interpreted only by the closed `backend` verifier. Keeping
/// the family tag beside the commitment prevents a valid checkpoint hash from
/// being routed to a different chain verifier through a domain-only lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct SccpNativeTrustAnchorV1 {
    /// Concrete native verifier that defines the anchor preimage and hash.
    pub backend: BridgeNativeProofBackendV1,
    /// Nonzero, role-separated hash of the governed native checkpoint.
    pub anchor_hash: [u8; 32],
    /// Backend-specific consensus-progress coordinate committed by `anchor_hash`.
    ///
    /// Ethereum lanes use a finalized beacon slot, while BSC/TRON lanes use a finalized block
    /// height. This is intentionally distinct from an Ethereum execution-block height carried by
    /// an admitted event proof.
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
    /// The next retained checkpoint is an inclusive upper boundary. The one-coordinate overlap lets
    /// BSC/TRON prove the boundary block while the successor checkpoint itself becomes usable.
    /// Without a successor the current checkpoint remains open-ended.
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
/// The SCCP crate owns and validates the typed envelope because it owns the chain-specific verifier
/// DTOs. The data model stores that canonical encoding once, paired with a closed backend
/// identifier; it does not disguise native consensus evidence as a transparent ZK proof or place it
/// inside a caller-labelled [`ProofBox`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
/// An unknown or caller-labelled backend is unrepresentable. The SCCP cryptographic implementation
/// additionally verifies that the canonical artifact's inner family agrees with this outer tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
    /// TON Groth16 verifier over BLS12-381 wrapping the governed Taira statement.
    #[codec(index = 2)]
    #[norito(rename = "ton_groth16_bls12381_v1")]
    TonGroth16Bls12381,
}
impl BridgeSccpDestinationProofBackendV1 {
    /// Return the stable production verifier label used in proof diagnostics.
    #[must_use]
    pub const fn backend_label(self) -> &'static str {
        match self {
            Self::EvmGroth16Bn254 => "evm-groth16-bn254-v1",
            Self::TronGroth16Bn254 => "tron-groth16-bn254-v1",
            Self::TonGroth16Bls12381 => "ton-groth16-bls12381-v1",
        }
    }
    /// Return whether this closed verifier backend belongs to an exact
    /// external destination profile.
    #[must_use]
    pub const fn supports_destination_network(self, target: SccpNetworkV1) -> bool {
        matches!(
            (self, target),
            (
                Self::EvmGroth16Bn254,
                SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet
            ) | (Self::TronGroth16Bn254, SccpNetworkV1::TronMainnet)
                | (Self::TonGroth16Bls12381, SccpNetworkV1::TonMainnet)
        )
    }
}
/// Canonically encoded production SCCP destination-proof artifact.
///
/// This closed container prevents production SCCP delivery from being routed
/// through generic [`ProofBox`] backend strings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
/// Current schema version of [`BridgeFinalityProof`].
pub const BRIDGE_FINALITY_PROOF_VERSION_V2: u8 = 2;
/// Current schema version of [`BridgeFinalityAttestationBodyV1`].
pub const BRIDGE_FINALITY_ATTESTATION_VERSION_V1: u8 = 1;
/// Domain separating a Torii finality attestation from every other node signature.
pub const BRIDGE_FINALITY_ATTESTATION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha:bridge-finality-attestation:v1\0";
/// Exact Sumeragi-v2 finality proof for one Iroha block.
///
/// The durable finality artifact is the single source of consensus context, height, block hash,
/// roster powers, quorum, subject, and commit certificate. No legacy certificate projection or
/// duplicate proof-controlled consensus field is carried alongside it.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeFinalityProof {
    /// Proof schema version. The first release requires [`BRIDGE_FINALITY_PROOF_VERSION_V2`].
    pub version: u8,
    /// Block header for the finalized block.
    pub block_header: crate::block::BlockHeader,
    /// Exact immutable finality artifact persisted by the Sumeragi-v2 apply path.
    pub finality_artifact: crate::block::consensus_v2::finality::V2FinalityArtifact,
}
/// Exact challenge-bound statement signed by one Torii node for a durable-tip capture.
///
/// `genesis_block_hash` is the first entry of the same committed state snapshot whose
/// durable tip produced `finality_proof`. The challenge is supplied by the caller and
/// prevents a previously signed capture from being replayed into a later audit run.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeFinalityAttestationBodyV1 {
    /// Attestation schema version.
    pub version: u8,
    /// Unpredictable caller challenge, required to be non-zero.
    pub challenge: [u8; 32],
    /// Exact genesis-derived network identity repeated for explicit signed routing identity.
    pub network_id: NetworkId,
    /// Canonical identity of the node which signs this body.
    pub node_id: crate::peer::PeerId,
    /// Hash of the canonical encoded `node_id`.
    pub node_fingerprint: iroha_crypto::Hash,
    /// Actual committed block hash at height one in the captured state snapshot.
    pub genesis_block_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
    /// Exact Kura-backed finality proof for the committed genesis block.
    ///
    /// This binds the genesis execution commitment and post-state root, not only
    /// the signed block header/payload identity.
    pub genesis_finality_proof: BridgeFinalityProof,
    /// Authoritative reducer-owned status captured for the same durable tip.
    pub status: crate::block::consensus_v2::SumeragiV2Status,
    /// Exact current-source proof for that durable tip.
    pub finality_proof: BridgeFinalityProof,
}
impl BridgeFinalityAttestationBodyV1 {
    /// Return the domain-separated typed digest signed by the reporting node.
    #[must_use]
    pub fn signing_hash(&self) -> iroha_crypto::HashOf<Self> {
        let encoded = self.encode();
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new_from_chunks(&[
            BRIDGE_FINALITY_ATTESTATION_SIGNATURE_DOMAIN_V1,
            &encoded,
        ]))
    }
    /// Validate all non-cryptographic duplicate bindings inside the signed body.
    ///
    /// # Errors
    ///
    /// Returns [`BridgeFinalityAttestationValidationError`] when the version,
    /// challenge, node identity, reducer status, or embedded proof disagree.
    pub fn validate_consistency(&self) -> Result<(), BridgeFinalityAttestationValidationError> {
        use crate::block::consensus_v2::PROTOCOL_VERSION;
        if self.version != BRIDGE_FINALITY_ATTESTATION_VERSION_V1 {
            return Err(
                BridgeFinalityAttestationValidationError::UnsupportedAttestationVersion {
                    expected: BRIDGE_FINALITY_ATTESTATION_VERSION_V1,
                    actual: self.version,
                },
            );
        }
        if self.challenge.iter().all(|byte| *byte == 0) {
            return Err(BridgeFinalityAttestationValidationError::ZeroChallenge);
        }
        let expected_node_fingerprint = iroha_crypto::Hash::new(self.node_id.encode());
        if self.node_fingerprint != expected_node_fingerprint {
            return Err(BridgeFinalityAttestationValidationError::NodeFingerprintMismatch);
        }
        if self.status.node_fingerprint != self.node_fingerprint {
            return Err(BridgeFinalityAttestationValidationError::StatusNodeMismatch);
        }
        self.status
            .validate()
            .map_err(|_| BridgeFinalityAttestationValidationError::InvalidStatus)?;
        if self.status.restart_required {
            return Err(BridgeFinalityAttestationValidationError::RestartRequired);
        }
        if self.status.protocol_version != PROTOCOL_VERSION {
            return Err(BridgeFinalityAttestationValidationError::ProtocolVersionMismatch);
        }
        validate_bridge_finality_proof_structure(&self.genesis_finality_proof, &self.network_id)
            .map_err(BridgeFinalityAttestationValidationError::InvalidGenesisProof)?;
        let genesis_artifact = &self.genesis_finality_proof.finality_artifact;
        if genesis_artifact.protocol_version != self.status.protocol_version {
            return Err(BridgeFinalityAttestationValidationError::ProtocolVersionMismatch);
        }
        if genesis_artifact.height != 1 {
            return Err(BridgeFinalityAttestationValidationError::GenesisProofHeightMismatch);
        }
        if genesis_artifact.block_hash != self.genesis_block_hash {
            return Err(BridgeFinalityAttestationValidationError::GenesisProofBlockMismatch);
        }
        validate_bridge_finality_proof_structure(&self.finality_proof, &self.network_id)
            .map_err(BridgeFinalityAttestationValidationError::InvalidProof)?;
        let artifact = &self.finality_proof.finality_artifact;
        if artifact.height == 1 && self.genesis_finality_proof != self.finality_proof {
            return Err(BridgeFinalityAttestationValidationError::HeightOneProofMismatch);
        }
        if artifact.protocol_version != self.status.protocol_version {
            return Err(BridgeFinalityAttestationValidationError::ProtocolVersionMismatch);
        }
        if self.status.last_committed_height != artifact.height {
            return Err(BridgeFinalityAttestationValidationError::StatusHeightMismatch);
        }
        if self.status.last_committed_subject.as_ref() != Some(&artifact.subject) {
            return Err(BridgeFinalityAttestationValidationError::StatusSubjectMismatch);
        }
        let Some(status_commit) = self.status.last_commit_qc.as_ref() else {
            return Err(BridgeFinalityAttestationValidationError::StatusCommitMissing);
        };
        if status_commit.certificate != artifact.commit_qc.as_ref() {
            return Err(BridgeFinalityAttestationValidationError::StatusCommitMismatch);
        }
        Ok(())
    }
}
/// One node's signature over an exact challenge-bound durable-tip statement.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeFinalityAttestationV1 {
    /// Complete signed statement.
    pub body: BridgeFinalityAttestationBodyV1,
    /// Signature made by `body.node_id` over `body.signing_hash()`.
    pub signature: iroha_crypto::SignatureOf<BridgeFinalityAttestationBodyV1>,
}
impl BridgeFinalityAttestationV1 {
    /// Validate the signed body and its reporting-node signature.
    ///
    /// # Errors
    ///
    /// Returns [`BridgeFinalityAttestationValidationError`] for an inconsistent
    /// body or a signature which does not verify under the declared node key.
    pub fn verify(&self) -> Result<(), BridgeFinalityAttestationValidationError> {
        self.body.validate_consistency()?;
        self.signature
            .verify_hash(self.body.node_id.public_key(), self.body.signing_hash())
            .map_err(|_| BridgeFinalityAttestationValidationError::InvalidNodeSignature)
    }
}
/// Failure while validating a challenge-bound node finality attestation.
#[allow(variant_size_differences)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BridgeFinalityAttestationValidationError {
    /// Attestation schema version is unsupported.
    #[error("bridge finality attestation version {actual} is unsupported; expected {expected}")]
    UnsupportedAttestationVersion {
        /// Supported version.
        expected: u8,
        /// Version carried in the signed body.
        actual: u8,
    },
    /// A zero challenge is replayable and therefore forbidden.
    #[error("bridge finality attestation challenge must be non-zero")]
    ZeroChallenge,
    /// The signed node fingerprint is not the hash of the canonical node id.
    #[error("bridge finality attestation node fingerprint does not match node id")]
    NodeFingerprintMismatch,
    /// The authoritative status belongs to another node.
    #[error("bridge finality attestation status belongs to another node")]
    StatusNodeMismatch,
    /// The reducer status is structurally invalid.
    #[error("bridge finality attestation contains an invalid Sumeragi-v2 status")]
    InvalidStatus,
    /// The reporting node is fail-stopped.
    #[error("bridge finality attestation status requires a node restart")]
    RestartRequired,
    /// Status and proof do not use the compiled current protocol.
    #[error("bridge finality attestation protocol versions do not match")]
    ProtocolVersionMismatch,
    /// The exact proof is structurally invalid or belongs to another network.
    #[error("bridge finality attestation proof is invalid: {0}")]
    InvalidProof(BridgeFinalityVerifyError),
    /// The height-one proof is structurally invalid or belongs to another network.
    #[error("bridge finality attestation genesis proof is invalid: {0}")]
    InvalidGenesisProof(BridgeFinalityVerifyError),
    /// The genesis proof is not for height one.
    #[error("bridge finality attestation genesis proof is not for height one")]
    GenesisProofHeightMismatch,
    /// The genesis proof does not authenticate the declared committed genesis hash.
    #[error("bridge finality attestation genesis proof block does not match genesis block hash")]
    GenesisProofBlockMismatch,
    /// At height one, genesis and tip must be the exact same durable Kura proof.
    #[error("bridge finality attestation height-one genesis and tip proofs do not match exactly")]
    HeightOneProofMismatch,
    /// Status and proof name different committed heights.
    #[error("bridge finality attestation status and proof heights do not match")]
    StatusHeightMismatch,
    /// Status and proof name different committed block subjects.
    #[error("bridge finality attestation status and proof subjects do not match")]
    StatusSubjectMismatch,
    /// The status does not expose its latest authenticated durable `CommitQC`.
    #[error("bridge finality attestation status has no durable CommitQC")]
    StatusCommitMissing,
    /// Status and proof carry different exact `CommitQCs`.
    #[error("bridge finality attestation status and proof CommitQCs do not match")]
    StatusCommitMismatch,
    /// The signature does not verify under the declared canonical node key.
    #[error("bridge finality attestation node signature is invalid")]
    InvalidNodeSignature,
}
/// Commitment covering a block hash and its exact Sumeragi-v2 context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeCommitment {
    /// Exact genesis-derived network identity to prevent cross-network replay.
    pub network_id: NetworkId,
    /// Typed hash of the complete immutable height context that finalized the block.
    pub height_context_id: crate::block::consensus_v2::HeightContextId,
    /// Block height bound into the commitment.
    pub block_height: u64,
    /// Block hash bound into the commitment.
    pub block_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
}
/// Bundle containing a compact commitment and its exact typed finality proof.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct BridgeFinalityBundle {
    /// Commitment binding the block hash and immutable height context.
    pub commitment: BridgeCommitment,
    /// Exact typed finality proof authenticated by the bundle.
    pub finality_proof: BridgeFinalityProof,
}
/// Internal consistency failure for a bridge finality bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BridgeFinalityBundleValidationError {
    /// Commitment and proof carry different network identities.
    #[error("bridge commitment network id does not match its finality proof")]
    NetworkIdMismatch,
    /// Commitment and proof carry different immutable context identifiers.
    #[error("bridge commitment context id does not match its finality proof")]
    ContextIdMismatch,
    /// Commitment and proof carry different block heights.
    #[error("bridge commitment height does not match its finality proof")]
    BlockHeightMismatch,
    /// Commitment and proof carry different block hashes.
    #[error("bridge commitment block hash does not match its finality proof")]
    BlockHashMismatch,
}
impl BridgeFinalityBundle {
    /// Validate the exact commitment/proof bindings.
    ///
    /// # Errors
    ///
    /// Returns [`BridgeFinalityBundleValidationError`] when any duplicate
    /// network, context, height, or block-hash binding differs.
    pub fn validate_consistency(&self) -> Result<(), BridgeFinalityBundleValidationError> {
        let artifact = &self.finality_proof.finality_artifact;
        if self.commitment.network_id != artifact.height_context.network_id {
            return Err(BridgeFinalityBundleValidationError::NetworkIdMismatch);
        }
        if self.commitment.height_context_id != artifact.context_id() {
            return Err(BridgeFinalityBundleValidationError::ContextIdMismatch);
        }
        if self.commitment.block_height != artifact.height {
            return Err(BridgeFinalityBundleValidationError::BlockHeightMismatch);
        }
        if self.commitment.block_hash != artifact.block_hash {
            return Err(BridgeFinalityBundleValidationError::BlockHashMismatch);
        }
        Ok(())
    }
}
/// Errors surfaced when verifying bridge finality proofs.
#[allow(variant_size_differences)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BridgeFinalityVerifyError {
    /// Proof schema version is unsupported.
    #[error("bridge finality proof version {actual} is unsupported; expected {expected}")]
    UnsupportedProofVersion {
        /// Supported first-release version.
        expected: u8,
        /// Version carried by the proof.
        actual: u8,
    },
    /// Proof is bound to a different exact network identity.
    #[error("network id mismatch: expected {expected}, got {got}")]
    NetworkIdMismatch {
        /// Expected network identity.
        expected: NetworkId,
        /// Network identity carried inside the artifact.
        got: NetworkId,
    },
    /// Typed artifact failed its structural v2 bindings.
    #[error("invalid Sumeragi-v2 finality artifact: {0}")]
    InvalidArtifact(crate::block::consensus_v2::finality::V2FinalityValidationError),
    /// Block header height differs from the artifact height.
    #[error("block header height {header_height} does not match artifact height {artifact_height}")]
    BlockHeaderHeightMismatch {
        /// Height recomputed from the block header.
        header_height: u64,
        /// Height carried by the durable artifact.
        artifact_height: u64,
    },
    /// Block header hash differs from the artifact's canonical block hash.
    #[error("block header hash {header_hash:?} does not match artifact hash {artifact_hash:?}")]
    BlockHeaderHashMismatch {
        /// Hash recomputed from the block header.
        header_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
        /// Hash carried by the durable artifact.
        artifact_hash: iroha_crypto::HashOf<crate::block::BlockHeader>,
    },
    /// Block header predecessor differs from the finalized subject predecessor.
    #[error("block header predecessor does not match the finalized subject")]
    BlockHeaderParentMismatch,
    /// `CommitQC` decision round precedes the view committed by the block header.
    #[error(
        "finality decision round {decision_view} precedes block header construction view {header_view}"
    )]
    BlockHeaderDecisionRoundBeforeView {
        /// View-change index recomputed from the block header.
        header_view: u64,
        /// View carried by the exact `CommitQC` round.
        decision_view: u64,
    },
    /// V2 certificate/roster cryptography failed.
    #[error("Sumeragi-v2 finality cryptography failed: {0}")]
    CertificateVerification(
        crate::block::consensus_v2::finality::V2QuorumCertificateVerificationError,
    ),
    /// Verification was attempted without a trusted first-height context id.
    #[error("a trusted Sumeragi-v2 height-context id is required")]
    MissingContextAnchor,
    /// The first proof does not match the trusted height-context id.
    #[error("proof context {got:?} does not match trusted context {expected:?}")]
    UnexpectedContext {
        /// Trusted context identifier.
        expected: crate::block::consensus_v2::HeightContextId,
        /// Context identifier carried by the artifact.
        got: crate::block::consensus_v2::HeightContextId,
    },
    /// The next proof does not carry a valid certificate for the prior decision.
    #[error("successor proof is not anchored to the previously finalized decision")]
    ParentFinalityMismatch,
    /// The next proof changes frozen election inputs outside v2 transition rules.
    #[error("successor proof violates the Sumeragi-v2 context transition")]
    SuccessorContextMismatch,
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
/// Failure while verifying a complete bridge finality bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BridgeFinalityBundleVerifyError {
    /// Commitment metadata is not internally consistent with the exact proof.
    #[error(transparent)]
    InvalidCommitment(#[from] BridgeFinalityBundleValidationError),
    /// The exact Sumeragi-v2 proof failed network, context, transition, or cryptographic checks.
    #[error(transparent)]
    InvalidProof(#[from] BridgeFinalityVerifyError),
}
/// Stateful verifier for bridge finality proofs.
///
/// The first proof must match an explicitly trusted Sumeragi-v2 height-context id. Every later
/// proof must be the immediate, cryptographically linked successor of the last accepted artifact.
/// This preserves the exact equal-vote quorum, epoch transitions, and parent finality without
/// trusting a proof-controlled roster.
#[derive(Debug, Clone)]
pub struct BridgeFinalityVerifier {
    expected_network_id: NetworkId,
    trusted_context_id: Option<crate::block::consensus_v2::HeightContextId>,
    latest_proof: Option<BridgeFinalityProof>,
}
impl BridgeFinalityVerifier {
    /// Construct a verifier bound only to an exact network identity.
    ///
    /// [`Self::set_context_anchor`] must be called before the first proof can be accepted.
    #[must_use]
    pub fn new(expected_network_id: NetworkId) -> Self {
        Self {
            expected_network_id,
            trusted_context_id: None,
            latest_proof: None,
        }
    }
    /// Construct a verifier bound to a network and first trusted v2 context id.
    #[must_use]
    pub fn with_context(
        expected_network_id: NetworkId,
        trusted_context_id: crate::block::consensus_v2::HeightContextId,
    ) -> Self {
        Self {
            expected_network_id,
            trusted_context_id: Some(trusted_context_id),
            latest_proof: None,
        }
    }
    /// Replace the trusted first context and discard prior verifier progress.
    pub fn set_context_anchor(
        &mut self,
        trusted_context_id: crate::block::consensus_v2::HeightContextId,
    ) {
        self.trusted_context_id = Some(trusted_context_id);
        self.latest_proof = None;
    }
    /// Verify a bridge finality proof against the configured expectations.
    ///
    /// # Errors
    ///
    /// Returns [`BridgeFinalityVerifyError`] when the proof's version, network,
    /// artifact/header binding, context anchor, successor transition, quorum,
    /// `PoPs`, or aggregate signature is invalid.
    pub fn verify(&mut self, proof: &BridgeFinalityProof) -> Result<(), BridgeFinalityVerifyError> {
        validate_bridge_finality_proof_structure(proof, &self.expected_network_id)?;
        if let Some(previous) = self.latest_proof.as_ref() {
            let previous_height = previous.finality_artifact.height;
            let height = proof.finality_artifact.height;
            if height <= previous_height {
                return Err(BridgeFinalityVerifyError::StaleHeight {
                    latest: previous_height,
                    height,
                });
            }
            if height > previous_height.saturating_add(1) {
                return Err(BridgeFinalityVerifyError::AdvancedHeight {
                    latest: previous_height,
                    height,
                });
            }
            verify_successor_bridge_finality_proof(previous, proof)?;
        } else {
            let expected = self
                .trusted_context_id
                .ok_or(BridgeFinalityVerifyError::MissingContextAnchor)?;
            let got = proof.finality_artifact.context_id();
            if got != expected {
                return Err(BridgeFinalityVerifyError::UnexpectedContext { expected, got });
            }
        }
        proof
            .finality_artifact
            .verify()
            .map_err(BridgeFinalityVerifyError::CertificateVerification)?;
        self.latest_proof = Some(proof.clone());
        Ok(())
    }
    /// Verify a bundle's exact commitment bindings and advance this verifier
    /// with its embedded finality proof.
    ///
    /// # Errors
    ///
    /// Returns [`BridgeFinalityBundleVerifyError`] when either the commitment tuple or the embedded
    /// proof is invalid. Verifier progress is unchanged on every error path.
    pub fn verify_bundle(
        &mut self,
        bundle: &BridgeFinalityBundle,
    ) -> Result<(), BridgeFinalityBundleVerifyError> {
        bundle.validate_consistency()?;
        self.verify(&bundle.finality_proof)?;
        Ok(())
    }
}
/// Verify one exact bridge finality proof without maintaining successor state.
///
/// # Errors
///
/// Returns [`BridgeFinalityVerifyError`] when the version, network, header, durable artifact,
/// powered quorum, roster `PoPs`, or aggregate signature is invalid. Callers must separately pin
/// the artifact's [`crate::block::consensus_v2::finality::V2FinalityArtifact::context_id`] or use
/// [`BridgeFinalityVerifier`] when establishing trust.
pub fn verify_bridge_finality_proof(
    proof: &BridgeFinalityProof,
    expected_network_id: &NetworkId,
) -> Result<(), BridgeFinalityVerifyError> {
    validate_bridge_finality_proof_structure(proof, expected_network_id)?;
    proof
        .finality_artifact
        .verify()
        .map_err(BridgeFinalityVerifyError::CertificateVerification)
}
/// Verify one complete bridge finality bundle without maintaining successor state.
///
/// This checks the exact commitment/proof bindings, expected network identity, header/artifact
/// bindings, powered quorum, roster `PoPs`, and aggregate signature.
///
/// # Errors
///
/// Returns [`BridgeFinalityBundleVerifyError`] when the commitment or embedded proof is invalid.
pub fn verify_bridge_finality_bundle(
    bundle: &BridgeFinalityBundle,
    expected_network_id: &NetworkId,
) -> Result<(), BridgeFinalityBundleVerifyError> {
    bundle.validate_consistency()?;
    verify_bridge_finality_proof(&bundle.finality_proof, expected_network_id)?;
    Ok(())
}
fn validate_bridge_finality_proof_structure(
    proof: &BridgeFinalityProof,
    expected_network_id: &NetworkId,
) -> Result<(), BridgeFinalityVerifyError> {
    if proof.version != BRIDGE_FINALITY_PROOF_VERSION_V2 {
        return Err(BridgeFinalityVerifyError::UnsupportedProofVersion {
            expected: BRIDGE_FINALITY_PROOF_VERSION_V2,
            actual: proof.version,
        });
    }
    let artifact = &proof.finality_artifact;
    artifact
        .validate()
        .map_err(BridgeFinalityVerifyError::InvalidArtifact)?;
    if artifact.height_context.network_id != *expected_network_id {
        return Err(BridgeFinalityVerifyError::NetworkIdMismatch {
            expected: *expected_network_id,
            got: artifact.height_context.network_id,
        });
    }
    let header_height = proof.block_header.height().get();
    if header_height != artifact.height {
        return Err(BridgeFinalityVerifyError::BlockHeaderHeightMismatch {
            header_height,
            artifact_height: artifact.height,
        });
    }
    let header_hash = proof.block_header.hash();
    if header_hash != artifact.block_hash {
        return Err(BridgeFinalityVerifyError::BlockHeaderHashMismatch {
            header_hash,
            artifact_hash: artifact.block_hash,
        });
    }
    if proof.block_header.prev_block_hash() != artifact.subject.parent_block_hash {
        return Err(BridgeFinalityVerifyError::BlockHeaderParentMismatch);
    }
    let header_view = proof.block_header.view_change_index();
    if !artifact.decision_round_covers_header_view(&proof.block_header) {
        return Err(
            BridgeFinalityVerifyError::BlockHeaderDecisionRoundBeforeView {
                header_view,
                decision_view: artifact.commit_qc.round.view,
            },
        );
    }
    Ok(())
}
fn verify_successor_bridge_finality_proof(
    previous: &BridgeFinalityProof,
    current: &BridgeFinalityProof,
) -> Result<(), BridgeFinalityVerifyError> {
    use crate::block::consensus_v2::finality::verify_quorum_certificate_with_validator_pops;
    let parent = &previous.finality_artifact;
    let child = &current.finality_artifact;
    let context = &child.height_context;
    let Some(parent_qc) = context.parent_commit_qc.as_ref() else {
        return Err(BridgeFinalityVerifyError::ParentFinalityMismatch);
    };
    if context.network_id != parent.height_context.network_id
        || context.mode != parent.height_context.mode
        || context.da_layout != parent.height_context.da_layout
        || !parent_qc
            .as_ref()
            .same_commit_decision(parent.commit_qc.as_ref())
    {
        return Err(BridgeFinalityVerifyError::ParentFinalityMismatch);
    }
    let transition_matches = parent
        .height_context
        .next_epoch_snapshot
        .as_ref()
        .map_or_else(
            || {
                context.epoch == parent.height_context.epoch
                    && context.epoch_end_height == parent.height_context.epoch_end_height
                    && context.roster == parent.height_context.roster
                    && context.quorum == parent.height_context.quorum
                    && context.leader_seed == parent.height_context.leader_seed
                    && child.validator_set_pops.as_slice() == parent.validator_set_pops.as_slice()
            },
            |snapshot| {
                context.epoch == snapshot.epoch
                    && context.epoch_end_height == snapshot.epoch_end_height
                    && context.mode == snapshot.mode
                    && context.roster == snapshot.roster
                    && context.quorum == snapshot.quorum
                    && context.leader_seed == snapshot.leader_seed
                    && child.validator_set_pops.as_slice() == snapshot.validator_set_pops.as_slice()
            },
        );
    if !transition_matches {
        return Err(BridgeFinalityVerifyError::SuccessorContextMismatch);
    }
    verify_quorum_certificate_with_validator_pops(
        &parent.height_context,
        parent_qc,
        &parent.validator_set_pops,
    )
    .map_err(BridgeFinalityVerifyError::CertificateVerification)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{block::consensus_v2 as wire, peer::PeerId};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_primitives::numeric::Numeric;
    use iroha_version::DecodeAll;
    use std::num::NonZeroU64;
    fn test_network_id(seed: &str) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
            Hash::new(seed.as_bytes()),
        ))
    }
    fn mint_finality_roster(
        network_id: NetworkId,
        epoch: u64,
        roster: &[wire::ValidatorPower],
    ) -> crate::isi::kagemusha_v1::KagemushaMintFinalityEpochRosterV1 {
        use crate::isi::kagemusha_v1::{
            KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterV1,
            KagemushaMintFinalityValidatorKeysV1,
        };

        KagemushaMintFinalityEpochRosterV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            network_id,
            epoch,
            validators: roster
                .iter()
                .enumerate()
                .map(
                    |(index, validator)| KagemushaMintFinalityValidatorKeysV1 {
                        validator: validator.validator.clone(),
                        eq_proof_public_key: [u8::try_from(index + 1)
                            .expect("small fixture roster");
                            32],
                        ep_proof_public_key: [u8::try_from(index + 17)
                            .expect("small fixture roster");
                            32],
                    },
                )
                .collect(),
        }
    }
    fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm).unwrap_or_else(|err| {
            panic!("{algorithm:?} bridge fixture key generation should succeed: {err}")
        })
    }
    fn checked_bls_keypair() -> KeyPair {
        checked_random_keypair_with_algorithm(Algorithm::BlsNormal)
    }
    struct V2Fixture {
        proof: BridgeFinalityProof,
        keys: Vec<KeyPair>,
        successor_keys: Option<Vec<KeyPair>>,
    }
    fn make_v2_fixture(network_seed: &str) -> V2Fixture {
        make_v2_fixture_config(network_seed, &[1, 1, 1, 1], &[0, 1, 2], false)
    }
    fn make_v2_fixture_with_quorum(
        network_seed: &str,
        powers: &[u64],
        signer_indices: &[u32],
    ) -> V2Fixture {
        make_v2_fixture_config(network_seed, powers, signer_indices, false)
    }
    fn make_boundary_v2_fixture(network_seed: &str) -> V2Fixture {
        make_v2_fixture_config(network_seed, &[1, 1, 1, 1], &[0, 1, 2], true)
    }
    fn attestation_for_fixture(fixture: &V2Fixture) -> BridgeFinalityAttestationV1 {
        use crate::block::consensus_v2::{
            SumeragiV2BodyState, SumeragiV2CommitQcStatus, SumeragiV2HeightContextStatus,
            SumeragiV2LivenessStatus, SumeragiV2Status, SumeragiV2StatusPhase,
        };
        let artifact = &fixture.proof.finality_artifact;
        let context = &artifact.height_context;
        let signer = &fixture.keys[0];
        let node_id = PeerId::new(signer.public_key().clone());
        let signed_power = artifact
            .commit_qc
            .signers
            .iter()
            .map(|index| context.roster[usize::try_from(*index).expect("signer index")].power)
            .sum();
        let status = SumeragiV2Status {
            protocol_version: wire::PROTOCOL_VERSION,
            node_fingerprint: Hash::new(node_id.encode()),
            build_fingerprint: Hash::new(b"attestation fixture build"),
            config_fingerprint: Hash::new(b"attestation fixture config"),
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
                validator_count: u32::try_from(context.roster.len()).expect("validator count"),
                quorum: context.quorum,
            },
            last_commit_qc: Some(SumeragiV2CommitQcStatus {
                certificate: artifact.commit_qc.as_ref(),
                validator_count: u32::try_from(context.roster.len()).expect("validator count"),
                signer_count: u32::try_from(artifact.commit_qc.signers.len())
                    .expect("signer count"),
                min_signers: context.quorum.min_signers,
                signed_power,
                total_power: context.quorum.total_power,
            }),
            liveness: SumeragiV2LivenessStatus::default(),
        };
        let body = BridgeFinalityAttestationBodyV1 {
            version: BRIDGE_FINALITY_ATTESTATION_VERSION_V1,
            challenge: *Hash::new(b"unpredictable finality capture challenge").as_ref(),
            network_id: context.network_id,
            node_fingerprint: status.node_fingerprint,
            node_id,
            genesis_block_hash: fixture.proof.block_header.hash(),
            genesis_finality_proof: fixture.proof.clone(),
            status,
            finality_proof: fixture.proof.clone(),
        };
        let signature = SignatureOf::try_from_hash(signer.private_key(), body.signing_hash())
            .expect("sign finality attestation fixture");
        BridgeFinalityAttestationV1 { body, signature }
    }
    #[expect(
        clippy::too_many_lines,
        reason = "the self-contained fixture builds one cryptographically coherent v2 artifact"
    )]
    fn make_v2_fixture_config(
        network_seed: &str,
        powers: &[u64],
        signer_indices: &[u32],
        boundary: bool,
    ) -> V2Fixture {
        use crate::block::consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding, QuorumCertificate,
            ValidatorPower, Vote,
        };
        let mut keys = powers
            .iter()
            .map(|_| checked_bls_keypair())
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| {
            PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
        });
        let roster = keys
            .iter()
            .zip(powers)
            .map(|(key, power)| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: *power,
            })
            .collect::<Vec<_>>();
        let validator_set_pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("derive validator proof of possession")
            })
            .collect::<Vec<_>>();
        let network_id = test_network_id(network_seed);
        let current_mint_finality_roster = mint_finality_roster(network_id, 0, &roster);
        let mint_finality_epoch_id = current_mint_finality_roster
            .finality_epoch_id()
            .expect("valid fixture mint-finality roster");
        let mut header = crate::block::BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        header.set_confidential_features(Some(
            crate::confidential::ConfidentialFeatureDigest::new(
                Some([0x91; 32]),
                Some(1),
                Some(2),
                Some(3),
                Some([0x92; 32]),
            ),
        ));
        let (next_epoch_snapshot, successor_keys) = if boundary {
            let mut next_keys = powers
                .iter()
                .map(|_| checked_bls_keypair())
                .collect::<Vec<_>>();
            next_keys.sort_by(|left, right| {
                PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
            });
            let next_roster = next_keys
                .iter()
                .zip(powers)
                .map(|(key, power)| ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: *power,
                })
                .collect::<Vec<_>>();
            let next_pops = next_keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("derive next-epoch validator proof of possession")
                })
                .collect();
            let next_mint_finality_roster = mint_finality_roster(network_id, 1, &next_roster);
            let next_mint_finality_epoch_id = next_mint_finality_roster
                .finality_epoch_id()
                .expect("valid next-epoch fixture mint-finality roster");
            (
                Some(
                    crate::block::consensus_v2::finality::FinalizedNextEpochSnapshot {
                        epoch: 1,
                        kagemusha_mint_finality_epoch_id: next_mint_finality_epoch_id,
                        kagemusha_mint_finality_epoch_roster: next_mint_finality_roster,
                        epoch_end_height: 11,
                        mode: ConsensusMode::Npos,
                        quorum: DualQuorum::from_roster(&next_roster)
                            .expect("valid boundary next-epoch quorum"),
                        validator_set_pops: next_pops,
                        roster: next_roster,
                        leader_seed: [0x6B; 32],
                    },
                ),
                Some(next_keys),
            )
        } else {
            (None, None)
        };
        let context = HeightContext {
            network_id,
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            kagemusha_mint_finality_epoch_id: mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster: current_mint_finality_roster,
            epoch_end_height: if boundary { 1 } else { 10 },
            next_epoch_snapshot,
            mode: ConsensusMode::Npos,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid roster"),
            roster,
            nexus_amx_context_hash: Hash::new(b"bridge v2 test nexus context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
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
            parent_block_hash: None,
            block_hash: header.hash(),
            payload_hash: Hash::new(b"bridge v2 test payload"),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height: 1,
            view: 0,
        };
        let execution_commitment =
            crate::block::consensus_v2::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                Hash::new(b"bridge v2 parent state"),
                Hash::new(b"bridge v2 post state"),
                Hash::new(b"bridge v2 ordinary writes"),
                1,
                Hash::new(b"bridge v2 executed block wire"),
            );
        let mut commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: signer_indices.to_vec(),
            aggregate_signature: vec![1],
        };
        let preimage = Vote {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: signer_indices.first().copied().unwrap_or(0),
            signature: Vec::new(),
        }
        .signature_preimage();
        let signature_payloads = signer_indices
            .iter()
            .map(|index| {
                Signature::try_new(
                    keys[usize::try_from(*index).expect("fixture signer index")].private_key(),
                    &preimage,
                )
                .expect("sign v2 commit vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signature_payloads
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        commit_qc.aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                .expect("aggregate v2 commit votes");
        let artifact = crate::block::consensus_v2::finality::V2FinalityArtifact::new(
            context,
            subject,
            commit_qc,
            validator_set_pops,
        );
        V2Fixture {
            proof: BridgeFinalityProof {
                version: BRIDGE_FINALITY_PROOF_VERSION_V2,
                block_header: header,
                finality_artifact: artifact,
            },
            keys,
            successor_keys,
        }
    }
    fn make_successor_v2_proof(parent: &V2Fixture) -> BridgeFinalityProof {
        let parent_artifact = &parent.proof.finality_artifact;
        let (
            epoch,
            mint_finality_epoch_id,
            mint_finality_roster,
            epoch_end_height,
            mode,
            roster,
            validator_set_pops,
            quorum,
            leader_seed,
        ) = parent_artifact
            .height_context
            .next_epoch_snapshot
            .as_ref()
            .map_or_else(
                || {
                    (
                        parent_artifact.height_context.epoch,
                        parent_artifact
                            .height_context
                            .kagemusha_mint_finality_epoch_id,
                        parent_artifact
                            .height_context
                            .kagemusha_mint_finality_epoch_roster
                            .clone(),
                        parent_artifact.height_context.epoch_end_height,
                        parent_artifact.height_context.mode,
                        parent_artifact.height_context.roster.clone(),
                        parent_artifact.validator_set_pops.clone(),
                        parent_artifact.height_context.quorum,
                        parent_artifact.height_context.leader_seed,
                    )
                },
                |snapshot| {
                    (
                        snapshot.epoch,
                        snapshot.kagemusha_mint_finality_epoch_id,
                        snapshot.kagemusha_mint_finality_epoch_roster.clone(),
                        snapshot.epoch_end_height,
                        snapshot.mode,
                        snapshot.roster.clone(),
                        snapshot.validator_set_pops.clone(),
                        snapshot.quorum,
                        snapshot.leader_seed,
                    )
                },
            );
        let height = parent_artifact.height + 1;
        assert!(
            height < epoch_end_height,
            "fixture successor must not itself be an epoch boundary"
        );
        let header = crate::block::BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero successor height"),
            Some(parent_artifact.block_hash),
            None,
            None,
            0,
            0,
        );
        let context = wire::HeightContext {
            network_id: parent_artifact.height_context.network_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height,
            epoch,
            kagemusha_mint_finality_epoch_id: mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster: mint_finality_roster,
            epoch_end_height,
            next_epoch_snapshot: None,
            mode,
            parent_commit_qc: Some(parent_artifact.commit_qc.clone()),
            snapshot_bootstrap: None,
            quorum,
            roster,
            nexus_amx_context_hash: Hash::new(b"bridge v2 successor nexus context"),
            execution_policy_hash: parent_artifact.height_context.execution_policy_hash,
            da_layout: parent_artifact.height_context.da_layout,
            leader_seed,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: Some(parent_artifact.block_hash),
            block_hash: header.hash(),
            payload_hash: Hash::new(b"bridge v2 successor payload"),
        };
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height,
            view: 0,
        };
        let execution_commitment =
            wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                Hash::new(b"bridge v2 successor parent state"),
                Hash::new(b"bridge v2 successor post state"),
                Hash::new(b"bridge v2 successor ordinary writes"),
                1,
                Hash::new(b"bridge v2 successor executed block wire"),
            );
        let commit_qc = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let artifact = wire::finality::V2FinalityArtifact::new(
            context,
            subject,
            commit_qc,
            validator_set_pops,
        );
        let mut proof = BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header: header,
            finality_artifact: artifact,
        };
        let signing_keys = parent.successor_keys.as_deref().unwrap_or(&parent.keys);
        resign_v2_proof(&mut proof, signing_keys);
        proof
    }
    fn resign_v2_proof(proof: &mut BridgeFinalityProof, keys: &[KeyPair]) {
        let artifact = &mut proof.finality_artifact;
        artifact.commit_qc.round.context_id = artifact.height_context.id();
        artifact.commit_qc.proposal_round.context_id = artifact.height_context.id();
        let preimage = wire::Vote {
            round: artifact.commit_qc.round,
            proposal_round: artifact.commit_qc.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: artifact.subject,
            execution_commitment: artifact.commit_qc.execution_commitment,
            signer: artifact.commit_qc.signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = artifact
            .commit_qc
            .signers
            .iter()
            .map(|index| {
                Signature::try_new(
                    keys[usize::try_from(*index).expect("fixture signer index")].private_key(),
                    &preimage,
                )
                .expect("sign successor v2 commit vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        artifact.commit_qc.aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate successor v2 commit votes");
    }
    fn rebind_v2_proof_to_header(proof: &mut BridgeFinalityProof, keys: &[KeyPair]) {
        let block_hash = proof.block_header.hash();
        proof.finality_artifact.block_hash = block_hash;
        proof.finality_artifact.subject.block_hash = block_hash;
        proof.finality_artifact.commit_qc.subject = proof.finality_artifact.subject;
        resign_v2_proof(proof, keys);
        proof
            .finality_artifact
            .verify()
            .expect("rebound attack fixture remains internally cryptographically valid");
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
    #[expect(
        clippy::too_many_lines,
        reason = "one closed-surface test covers every backend and source-network pairing"
    )]
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
            (
                BridgeNativeProofBackendV1::TonMasterchain,
                "bridge/sccp/native/ton-masterchain-v1",
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
            SccpNetworkV1::BscMainnet,
            SccpNetworkV1::TronMainnet,
            SccpNetworkV1::TonMainnet,
        ];
        for &(backend, _) in &backends {
            for source in external_profiles {
                let expected = matches!(
                    (backend, source),
                    (
                        BridgeNativeProofBackendV1::EthereumBeacon,
                        SccpNetworkV1::EthereumMainnet
                    ) | (
                        BridgeNativeProofBackendV1::BscParlia,
                        SccpNetworkV1::BscMainnet
                    ) | (
                        BridgeNativeProofBackendV1::TronDpos,
                        SccpNetworkV1::TronMainnet
                    ) | (
                        BridgeNativeProofBackendV1::TonMasterchain,
                        SccpNetworkV1::TonMainnet
                    )
                );
                assert_eq!(backend.supports_source_network(source), expected);
            }
            assert!(!backend.supports_source_network(SccpNetworkV1::SoraTaira));
        }
        for unknown_tag in [4_u32, 5, u32::MAX] {
            let encoded = unknown_tag.encode();
            assert!(
                BridgeNativeProofBackendV1::decode_all(&mut encoded.as_slice()).is_err(),
                "unknown native backend tag {unknown_tag} unexpectedly decoded"
            );
        }
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
        for (backend, label) in [
            (
                BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
                "evm-groth16-bn254-v1",
            ),
            (
                BridgeSccpDestinationProofBackendV1::TronGroth16Bn254,
                "tron-groth16-bn254-v1",
            ),
            (
                BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381,
                "ton-groth16-bls12381-v1",
            ),
        ] {
            assert_eq!(backend.backend_label(), label);
            let encoded = backend.encode();
            assert_eq!(
                BridgeSccpDestinationProofBackendV1::decode_all(&mut encoded.as_slice())
                    .expect("closed destination backend decodes"),
                backend
            );
        }
        for backend in [
            BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
            BridgeSccpDestinationProofBackendV1::TronGroth16Bn254,
            BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381,
        ] {
            for network in [
                SccpNetworkV1::SoraTaira,
                SccpNetworkV1::EthereumMainnet,
                SccpNetworkV1::BscMainnet,
                SccpNetworkV1::TronMainnet,
                SccpNetworkV1::TonMainnet,
            ] {
                let expected = matches!(
                    (backend, network),
                    (
                        BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
                        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet
                    ) | (
                        BridgeSccpDestinationProofBackendV1::TronGroth16Bn254,
                        SccpNetworkV1::TronMainnet
                    ) | (
                        BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381,
                        SccpNetworkV1::TonMainnet
                    )
                );
                assert_eq!(backend.supports_destination_network(network), expected);
            }
        }
        for unknown_tag in [3_u32, 4, u32::MAX] {
            let unknown_backend = unknown_tag.encode();
            assert!(
                BridgeSccpDestinationProofBackendV1::decode_all(&mut unknown_backend.as_slice())
                    .is_err()
            );
        }
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
            amount: 42_u64.into(),
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        };
        let buf = r.encode();
        let dec = BridgeReceipt::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(r, dec);
    }
    #[derive(Encode)]
    struct ForgedBridgeReceipt {
        lane: LaneId,
        direction: Vec<u8>,
        source_tx: [u8; 32],
        dest_tx: Option<[u8; 32]>,
        proof_hash: [u8; 32],
        amount: Numeric,
        asset_id: Vec<u8>,
        recipient: Vec<u8>,
    }
    #[test]
    fn bridge_receipt_rejects_negative_numeric_amount() {
        let forged = ForgedBridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: None,
            proof_hash: [0x33; 32],
            amount: Numeric::new(-1_i32, 0),
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        };
        let encoded = forged.encode();
        assert!(
            BridgeReceipt::decode_all(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as a bridge amount"
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn bridge_receipt_json_rejects_unknown_fields() {
        let receipt = BridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: None,
            proof_hash: [0x33; 32],
            amount: 42_u64.into(),
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        };
        let canonical = norito::json::to_json(&receipt).expect("serialize bridge receipt JSON");
        assert_eq!(
            norito::json::from_json::<BridgeReceipt>(&canonical)
                .expect("canonical bridge receipt JSON decodes"),
            receipt
        );
        let hostile = canonical.replacen('{', "{\"adversarial_extension\":null,", 1);
        assert_ne!(hostile, canonical);
        assert!(
            norito::json::from_json::<BridgeReceipt>(&hostile).is_err(),
            "signed receipt JSON must reject unknown fields"
        );
    }
    #[test]
    fn sccp_outbound_message_key_roundtrip() {
        let key = SccpOutboundMessageKeyV1::new(
            SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::BscMainnet,
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
        let record = SccpOutboundPendingMessageRecordV1 {
            destination_binding_hash: [0x23; 32],
            route_configuration_hash: [0x25; 32],
            payload_hash: [0x24; 32],
            payload_bytes: vec![0x53, 0x43, 0x43, 0x50],
            recorded_at_height: 77,
            commitment_index: 0,
        };
        let buf = record.encode();
        let dec = SccpOutboundPendingMessageRecordV1::decode_all(&mut &buf[..]).expect("decode");
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
    fn bridge_finality_proof_roundtrip_preserves_exact_v2_artifact() {
        let fixture = make_v2_fixture("proof-chain");
        let encoded = fixture.proof.encode();
        let decoded = BridgeFinalityProof::decode_all(&mut encoded.as_slice()).expect("decode");
        assert_eq!(decoded, fixture.proof);
        decoded
            .finality_artifact
            .verify()
            .expect("roundtripped proof remains cryptographically valid");
    }
    #[test]
    fn bridge_finality_attestation_binds_challenge_node_genesis_status_and_proof() {
        let fixture = make_v2_fixture("attested-proof-chain");
        let attestation = attestation_for_fixture(&fixture);
        attestation.verify().expect("valid node attestation");
        let encoded = attestation.encode();
        let decoded = BridgeFinalityAttestationV1::decode_all(&mut encoded.as_slice())
            .expect("decode finality attestation");
        assert_eq!(decoded, attestation);
        decoded.verify().expect("roundtripped attestation verifies");
        let mut changed_challenge = attestation.clone();
        changed_challenge.body.challenge = *Hash::new(b"another capture challenge").as_ref();
        assert_eq!(
            changed_challenge.verify(),
            Err(BridgeFinalityAttestationValidationError::InvalidNodeSignature)
        );
        let mut changed_genesis = attestation.clone();
        changed_genesis.body.genesis_block_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"another committed genesis"));
        assert_eq!(
            changed_genesis.verify(),
            Err(BridgeFinalityAttestationValidationError::GenesisProofBlockMismatch)
        );
        let alternate = make_v2_fixture("attested-proof-chain");
        let mut substituted_genesis_proof = attestation;
        substituted_genesis_proof.body.genesis_block_hash = alternate.proof.block_header.hash();
        substituted_genesis_proof.body.genesis_finality_proof = alternate.proof;
        assert_eq!(
            substituted_genesis_proof.verify(),
            Err(BridgeFinalityAttestationValidationError::HeightOneProofMismatch)
        );
    }
    #[test]
    fn bridge_finality_attestation_rejects_replayable_or_mixed_node_status() {
        let fixture = make_v2_fixture("strict-attested-proof-chain");
        let attestation = attestation_for_fixture(&fixture);
        let mut zero_challenge = attestation.clone();
        zero_challenge.body.challenge = [0; 32];
        assert_eq!(
            zero_challenge.body.validate_consistency(),
            Err(BridgeFinalityAttestationValidationError::ZeroChallenge)
        );
        let mut mixed_node = attestation;
        mixed_node.body.status.node_fingerprint = Hash::new(b"another Torii node");
        assert_eq!(
            mixed_node.body.validate_consistency(),
            Err(BridgeFinalityAttestationValidationError::StatusNodeMismatch)
        );
    }
    #[cfg(feature = "json")]
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one table-driven test covers every nested consensus JSON boundary"
    )]
    fn bridge_finality_json_rejects_unknown_fields_at_every_consensus_boundary() {
        #[derive(Clone, Copy, Debug)]
        enum JsonPathStep {
            Field(&'static str),
            Index(usize),
        }
        fn insert_hostile_field(value: &mut norito::json::Value, path: &[JsonPathStep]) {
            let mut current = value;
            for step in path {
                current = match step {
                    JsonPathStep::Field(field) => {
                        let norito::json::Value::Object(object) = current else {
                            panic!("JSON path component `{field}` is not an object")
                        };
                        object
                            .get_mut(*field)
                            .unwrap_or_else(|| panic!("JSON path component `{field}` is absent"))
                    }
                    JsonPathStep::Index(index) => {
                        let norito::json::Value::Array(array) = current else {
                            panic!("JSON path component index {index} is not an array")
                        };
                        array
                            .get_mut(*index)
                            .unwrap_or_else(|| panic!("JSON path index {index} is absent"))
                    }
                };
            }
            let norito::json::Value::Object(object) = current else {
                panic!("hostile JSON target at {path:?} is not an object")
            };
            object.insert("adversarial_extension".into(), norito::json::Value::Null);
        }
        use JsonPathStep::{Field, Index};
        let fixture = make_boundary_v2_fixture("closed-finality-json");
        let canonical =
            norito::json::to_json(&fixture.proof).expect("serialize exact finality proof JSON");
        assert_eq!(
            norito::json::from_json::<BridgeFinalityProof>(&canonical)
                .expect("canonical exact finality JSON decodes"),
            fixture.proof
        );
        let paths = [
            ("proof", vec![]),
            ("block header", vec![Field("block_header")]),
            (
                "confidential feature digest",
                vec![Field("block_header"), Field("confidential_features")],
            ),
            ("artifact", vec![Field("finality_artifact")]),
            (
                "height context",
                vec![Field("finality_artifact"), Field("height_context")],
            ),
            (
                "current validator",
                vec![
                    Field("finality_artifact"),
                    Field("height_context"),
                    Field("roster"),
                    Index(0),
                ],
            ),
            (
                "current quorum",
                vec![
                    Field("finality_artifact"),
                    Field("height_context"),
                    Field("quorum"),
                ],
            ),
            (
                "consensus mode",
                vec![
                    Field("finality_artifact"),
                    Field("height_context"),
                    Field("mode"),
                ],
            ),
            (
                "data-availability layout",
                vec![
                    Field("finality_artifact"),
                    Field("height_context"),
                    Field("da_layout"),
                ],
            ),
            (
                "payload encoding",
                vec![
                    Field("finality_artifact"),
                    Field("height_context"),
                    Field("da_layout"),
                    Field("encoding"),
                ],
            ),
            (
                "next-epoch snapshot",
                vec![
                    Field("finality_artifact"),
                    Field("height_context"),
                    Field("next_epoch_snapshot"),
                ],
            ),
            (
                "next-epoch validator",
                vec![
                    Field("finality_artifact"),
                    Field("height_context"),
                    Field("next_epoch_snapshot"),
                    Field("roster"),
                    Index(0),
                ],
            ),
            (
                "next-epoch quorum",
                vec![
                    Field("finality_artifact"),
                    Field("height_context"),
                    Field("next_epoch_snapshot"),
                    Field("quorum"),
                ],
            ),
            (
                "artifact subject",
                vec![Field("finality_artifact"), Field("subject")],
            ),
            (
                "commit certificate",
                vec![Field("finality_artifact"), Field("commit_qc")],
            ),
            (
                "certificate round",
                vec![
                    Field("finality_artifact"),
                    Field("commit_qc"),
                    Field("round"),
                ],
            ),
            (
                "certificate phase",
                vec![
                    Field("finality_artifact"),
                    Field("commit_qc"),
                    Field("phase"),
                ],
            ),
            (
                "certificate subject",
                vec![
                    Field("finality_artifact"),
                    Field("commit_qc"),
                    Field("subject"),
                ],
            ),
            (
                "execution commitment",
                vec![
                    Field("finality_artifact"),
                    Field("commit_qc"),
                    Field("execution_commitment"),
                ],
            ),
        ];
        for (name, path) in paths {
            let mut hostile = norito::json::to_value(&fixture.proof)
                .expect("serialize exact finality proof value");
            insert_hostile_field(&mut hostile, &path);
            let hostile = norito::json::to_json(&hostile).expect("serialize hostile JSON value");
            assert!(
                norito::json::from_json::<BridgeFinalityProof>(&hostile).is_err(),
                "unknown field in {name} must fail closed"
            );
        }
    }
    #[test]
    fn bridge_finality_bundle_roundtrip_commits_to_exact_context() {
        let fixture = make_v2_fixture("bundle-chain");
        let proof = fixture.proof;
        let context_id = proof.finality_artifact.context_id();
        let bundle = BridgeFinalityBundle {
            commitment: BridgeCommitment {
                network_id: proof.finality_artifact.height_context.network_id,
                height_context_id: context_id,
                block_height: proof.finality_artifact.height,
                block_hash: proof.finality_artifact.block_hash,
            },
            finality_proof: proof,
        };
        let encoded = bundle.encode();
        let decoded = BridgeFinalityBundle::decode_all(&mut encoded.as_slice()).expect("decode");
        assert_eq!(decoded, bundle);
        assert_eq!(decoded.commitment.height_context_id, context_id);
        decoded
            .validate_consistency()
            .expect("exact bundle commitment matches its proof");
        verify_bridge_finality_bundle(
            &decoded,
            &decoded
                .finality_proof
                .finality_artifact
                .height_context
                .network_id,
        )
        .expect("stateless exact bundle verification succeeds");
        let mut verifier = BridgeFinalityVerifier::with_context(
            decoded
                .finality_proof
                .finality_artifact
                .height_context
                .network_id,
            context_id,
        );
        verifier
            .verify_bundle(&decoded)
            .expect("stateful exact bundle verification succeeds");
    }
    #[test]
    fn bridge_finality_bundle_rejects_every_duplicate_binding_substitution() {
        let fixture = make_v2_fixture("bundle-chain");
        let proof = fixture.proof;
        let bundle = BridgeFinalityBundle {
            commitment: BridgeCommitment {
                network_id: proof.finality_artifact.height_context.network_id,
                height_context_id: proof.finality_artifact.context_id(),
                block_height: proof.finality_artifact.height,
                block_hash: proof.finality_artifact.block_hash,
            },
            finality_proof: proof,
        };
        let mut wrong_network = bundle.clone();
        wrong_network.commitment.network_id = test_network_id("other-network");
        assert_eq!(
            wrong_network.validate_consistency(),
            Err(BridgeFinalityBundleValidationError::NetworkIdMismatch)
        );
        let mut wrong_context = bundle.clone();
        wrong_context.commitment.height_context_id = make_v2_fixture("other-chain")
            .proof
            .finality_artifact
            .context_id();
        assert_eq!(
            wrong_context.validate_consistency(),
            Err(BridgeFinalityBundleValidationError::ContextIdMismatch)
        );
        let mut wrong_height = bundle.clone();
        wrong_height.commitment.block_height += 1;
        assert_eq!(
            wrong_height.validate_consistency(),
            Err(BridgeFinalityBundleValidationError::BlockHeightMismatch)
        );
        let mut wrong_hash = bundle;
        wrong_hash.commitment.block_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"substituted bundle commitment block hash"));
        assert_eq!(
            wrong_hash.validate_consistency(),
            Err(BridgeFinalityBundleValidationError::BlockHashMismatch)
        );
    }
    #[test]
    fn verifier_accepts_equal_vote_npos_quorum_with_context_anchor() {
        let fixture = make_v2_fixture("chain-a");
        let proof = fixture.proof;
        let mut verifier = BridgeFinalityVerifier::with_context(
            proof.finality_artifact.height_context.network_id,
            proof.finality_artifact.context_id(),
        );
        verifier.verify(&proof).expect("valid exact v2 proof");
    }
    #[test]
    fn verifier_requires_an_explicit_context_anchor() {
        let fixture = make_v2_fixture("chain-a");
        let mut verifier =
            BridgeFinalityVerifier::new(fixture.proof.finality_artifact.height_context.network_id);
        assert!(matches!(
            verifier.verify(&fixture.proof),
            Err(BridgeFinalityVerifyError::MissingContextAnchor)
        ));
    }
    #[test]
    fn verifier_rejects_version_network_header_height_and_hash_drift() {
        let fixture = make_v2_fixture("chain-a");
        let expected_network = fixture.proof.finality_artifact.height_context.network_id;
        let context_id = fixture.proof.finality_artifact.context_id();
        let mut wrong_version = fixture.proof.clone();
        wrong_version.version = BRIDGE_FINALITY_PROOF_VERSION_V2 + 1;
        let mut verifier = BridgeFinalityVerifier::with_context(expected_network, context_id);
        assert!(matches!(
            verifier.verify(&wrong_version),
            Err(BridgeFinalityVerifyError::UnsupportedProofVersion { .. })
        ));
        let mut legacy_v1 = fixture.proof.clone();
        legacy_v1.version = 1;
        let mut verifier = BridgeFinalityVerifier::with_context(expected_network, context_id);
        assert!(matches!(
            verifier.verify(&legacy_v1),
            Err(BridgeFinalityVerifyError::UnsupportedProofVersion {
                expected: BRIDGE_FINALITY_PROOF_VERSION_V2,
                actual: 1,
            })
        ));
        let mut verifier =
            BridgeFinalityVerifier::with_context(test_network_id("other-chain"), context_id);
        assert!(matches!(
            verifier.verify(&fixture.proof),
            Err(BridgeFinalityVerifyError::NetworkIdMismatch { .. })
        ));
        let mut wrong_height = fixture.proof.clone();
        wrong_height.block_header = crate::block::BlockHeader::new(
            NonZeroU64::new(2).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut verifier = BridgeFinalityVerifier::with_context(expected_network, context_id);
        assert!(matches!(
            verifier.verify(&wrong_height),
            Err(BridgeFinalityVerifyError::BlockHeaderHeightMismatch { .. })
        ));
        let mut wrong_hash = fixture.proof.clone();
        wrong_hash.block_header = crate::block::BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            Some(HashOf::from_untyped_unchecked(Hash::new(
                b"different result root",
            ))),
            0,
            0,
        );
        let mut verifier = BridgeFinalityVerifier::with_context(expected_network, context_id);
        assert!(matches!(
            verifier.verify(&wrong_hash),
            Err(BridgeFinalityVerifyError::BlockHeaderHashMismatch { .. })
        ));
    }
    #[test]
    fn verifier_rejects_self_consistent_header_parent_and_view_substitutions() {
        let mut parent_attack = make_v2_fixture("chain-a");
        parent_attack
            .proof
            .block_header
            .set_prev_block_hash(Some(HashOf::from_untyped_unchecked(Hash::new(
                b"forged genesis predecessor",
            ))));
        rebind_v2_proof_to_header(&mut parent_attack.proof, &parent_attack.keys);
        assert_eq!(
            parent_attack
                .proof
                .finality_artifact
                .validate_for_header(&parent_attack.proof.block_header),
            Err(
                crate::block::consensus_v2::finality::V2FinalityValidationError::AssociatedParentBlockHashMismatch
            )
        );
        let mut verifier = BridgeFinalityVerifier::with_context(
            parent_attack
                .proof
                .finality_artifact
                .height_context
                .network_id,
            parent_attack.proof.finality_artifact.context_id(),
        );
        assert_eq!(
            verifier.verify(&parent_attack.proof),
            Err(BridgeFinalityVerifyError::BlockHeaderParentMismatch)
        );
        let mut view_attack = make_v2_fixture("chain-a");
        view_attack.proof.block_header.set_view_change_index(7);
        view_attack.proof.finality_artifact.commit_qc.round.view = 6;
        view_attack.proof.finality_artifact.commit_qc.proposal_round =
            view_attack.proof.finality_artifact.commit_qc.round;
        rebind_v2_proof_to_header(&mut view_attack.proof, &view_attack.keys);
        assert_eq!(
            view_attack
                .proof
                .finality_artifact
                .validate_for_header(&view_attack.proof.block_header),
            Err(
                crate::block::consensus_v2::finality::V2FinalityValidationError::DecisionRoundBeforeBlockView {
                    decision: 6,
                    block: 7,
                }
            )
        );
        let mut verifier = BridgeFinalityVerifier::with_context(
            view_attack
                .proof
                .finality_artifact
                .height_context
                .network_id,
            view_attack.proof.finality_artifact.context_id(),
        );
        assert_eq!(
            verifier.verify(&view_attack.proof),
            Err(
                BridgeFinalityVerifyError::BlockHeaderDecisionRoundBeforeView {
                    header_view: 7,
                    decision_view: 6,
                }
            )
        );
    }
    #[test]
    fn verifier_accepts_locked_block_decided_after_unchanged_reproposal() {
        let mut delayed = make_v2_fixture("chain-a");
        delayed.proof.block_header.set_view_change_index(3);
        delayed.proof.finality_artifact.commit_qc.round.view = 5;
        delayed.proof.finality_artifact.commit_qc.proposal_round =
            delayed.proof.finality_artifact.commit_qc.round;
        rebind_v2_proof_to_header(&mut delayed.proof, &delayed.keys);
        delayed
            .proof
            .finality_artifact
            .validate_for_header(&delayed.proof.block_header)
            .expect("a later-round certificate is valid for the unchanged locked block");
        let mut verifier = BridgeFinalityVerifier::with_context(
            delayed.proof.finality_artifact.height_context.network_id,
            delayed.proof.finality_artifact.context_id(),
        );
        verifier
            .verify(&delayed.proof)
            .expect("bridge verification accepts an unchanged later-round re-proposal");
    }
    #[test]
    fn successor_rejects_a_resigned_wrong_header_predecessor() {
        let parent = make_v2_fixture("chain-a");
        let mut child = make_successor_v2_proof(&parent);
        child
            .block_header
            .set_prev_block_hash(Some(HashOf::from_untyped_unchecked(Hash::new(
                b"unrelated predecessor",
            ))));
        rebind_v2_proof_to_header(&mut child, &parent.keys);
        let mut verifier = BridgeFinalityVerifier::with_context(
            parent.proof.finality_artifact.height_context.network_id,
            parent.proof.finality_artifact.context_id(),
        );
        verifier
            .verify(&parent.proof)
            .expect("valid parent establishes verifier progress");
        assert_eq!(
            verifier.verify(&child),
            Err(BridgeFinalityVerifyError::BlockHeaderParentMismatch)
        );
    }
    #[test]
    fn verifier_rejects_missing_or_invalid_roster_pops() {
        let fixture = make_v2_fixture("chain-a");
        let context = fixture.proof.finality_artifact.height_context.clone();
        let context_id = context.id();
        let mut missing = fixture.proof.clone();
        missing.finality_artifact.validator_set_pops.pop();
        let mut verifier = BridgeFinalityVerifier::with_context(context.network_id, context_id);
        assert!(matches!(
            verifier.verify(&missing),
            Err(BridgeFinalityVerifyError::InvalidArtifact(
                crate::block::consensus_v2::finality::V2FinalityValidationError::ProofOfPossessionCount { .. }
            ))
        ));
        let other = make_v2_fixture("chain-b");
        let mut invalid = fixture.proof;
        invalid.finality_artifact.validator_set_pops[3] =
            other.proof.finality_artifact.validator_set_pops[0].clone();
        let mut verifier = BridgeFinalityVerifier::with_context(context.network_id, context_id);
        assert!(matches!(
            verifier.verify(&invalid),
            Err(BridgeFinalityVerifyError::CertificateVerification(
                crate::block::consensus_v2::finality::V2QuorumCertificateVerificationError::InvalidProofOfPossession { index: 3 }
            ))
        ));
    }
    #[test]
    fn verifier_rejects_invalid_aggregate_signature() {
        let mut fixture = make_v2_fixture("chain-a");
        let context = fixture.proof.finality_artifact.height_context.clone();
        let mut oversized = fixture.proof.clone();
        oversized.finality_artifact.commit_qc.aggregate_signature =
            vec![0x7F; crate::block::consensus_v2::MAX_CONSENSUS_SIGNATURE_BYTES + 1];
        let mut verifier = BridgeFinalityVerifier::with_context(context.network_id, context.id());
        assert!(matches!(
            verifier.verify(&oversized),
            Err(BridgeFinalityVerifyError::InvalidArtifact(
                crate::block::consensus_v2::finality::V2FinalityValidationError::InvalidCommitCertificate(
                    crate::block::consensus_v2::ValidationError::SignatureTooLarge
                )
            ))
        ));
        fixture
            .proof
            .finality_artifact
            .commit_qc
            .aggregate_signature[0] ^= 0x80;
        let context_id = context.id();
        let mut verifier = BridgeFinalityVerifier::with_context(context.network_id, context_id);
        assert!(matches!(
            verifier.verify(&fixture.proof),
            Err(BridgeFinalityVerifyError::CertificateVerification(
                crate::block::consensus_v2::finality::V2QuorumCertificateVerificationError::InvalidAggregateSignature
            ))
        ));
    }
    #[test]
    fn boundary_transition_cannot_be_replaced_without_old_roster_signatures() {
        use crate::block::consensus_v2::finality::{
            V2FinalityValidationError, V2QuorumCertificateVerificationError,
        };
        let fixture = make_boundary_v2_fixture("chain-a");
        let original_context = fixture.proof.finality_artifact.height_context.clone();
        let mut stale_context_id = fixture.proof.clone();
        stale_context_id
            .finality_artifact
            .height_context
            .next_epoch_snapshot
            .as_mut()
            .expect("boundary snapshot")
            .leader_seed[0] ^= 0x80;
        let mut verifier = BridgeFinalityVerifier::with_context(
            original_context.network_id,
            original_context.id(),
        );
        assert!(matches!(
            verifier.verify(&stale_context_id),
            Err(BridgeFinalityVerifyError::InvalidArtifact(
                V2FinalityValidationError::CertificateContextMismatch
            ))
        ));
        let mut forged_context_id = fixture.proof;
        forged_context_id
            .finality_artifact
            .height_context
            .next_epoch_snapshot
            .as_mut()
            .expect("boundary snapshot")
            .leader_seed[0] ^= 0x80;
        let replacement_context_id = forged_context_id.finality_artifact.context_id();
        forged_context_id
            .finality_artifact
            .commit_qc
            .round
            .context_id = replacement_context_id;
        forged_context_id
            .finality_artifact
            .commit_qc
            .proposal_round
            .context_id = replacement_context_id;
        let mut verifier = BridgeFinalityVerifier::with_context(
            original_context.network_id,
            forged_context_id.finality_artifact.context_id(),
        );
        assert!(matches!(
            verifier.verify(&forged_context_id),
            Err(BridgeFinalityVerifyError::CertificateVerification(
                V2QuorumCertificateVerificationError::InvalidAggregateSignature
            ))
        ));
    }
    #[test]
    fn successor_rejects_a_resigned_boundary_epoch_end_substitution() {
        let parent = make_boundary_v2_fixture("chain-a");
        let child = make_successor_v2_proof(&parent);
        let snapshot = parent
            .proof
            .finality_artifact
            .height_context
            .next_epoch_snapshot
            .as_ref()
            .expect("boundary snapshot");
        assert_ne!(
            snapshot.roster, parent.proof.finality_artifact.height_context.roster,
            "boundary fixture must exercise a genuinely rotated BLS roster"
        );
        let network_id = parent.proof.finality_artifact.height_context.network_id;
        let context_anchor = parent.proof.finality_artifact.context_id();
        let mut verifier = BridgeFinalityVerifier::with_context(network_id, context_anchor);
        verifier
            .verify(&parent.proof)
            .expect("authenticated boundary parent");
        verifier
            .verify(&child)
            .expect("exact authenticated successor schedule");
        let mut substituted = child;
        substituted
            .finality_artifact
            .height_context
            .epoch_end_height += 1;
        resign_v2_proof(
            &mut substituted,
            parent
                .successor_keys
                .as_deref()
                .expect("rotated successor keys"),
        );
        verify_bridge_finality_proof(&substituted, &network_id)
            .expect("substituted child is independently self-consistent");
        substituted.finality_artifact.commit_qc.aggregate_signature[0] ^= 0x80;
        let mut verifier = BridgeFinalityVerifier::with_context(network_id, context_anchor);
        verifier
            .verify(&parent.proof)
            .expect("authenticated boundary parent");
        assert_eq!(
            verifier.verify(&substituted),
            Err(BridgeFinalityVerifyError::SuccessorContextMismatch),
            "cheap authenticated-schedule rejection must precede hostile BLS work"
        );
    }
    #[test]
    fn rotated_boundary_rejects_old_permuted_pops_and_old_key_signatures() {
        use crate::block::consensus_v2::finality::V2QuorumCertificateVerificationError;
        let parent = make_boundary_v2_fixture("rotated-chain");
        let child = make_successor_v2_proof(&parent);
        let network_id = parent.proof.finality_artifact.height_context.network_id;
        let anchor = parent.proof.finality_artifact.context_id();
        let mut old_pops = child.clone();
        old_pops.finality_artifact.validator_set_pops =
            parent.proof.finality_artifact.validator_set_pops.clone();
        let mut verifier = BridgeFinalityVerifier::with_context(network_id, anchor);
        verifier.verify(&parent.proof).expect("boundary parent");
        assert_eq!(
            verifier.verify(&old_pops),
            Err(BridgeFinalityVerifyError::SuccessorContextMismatch)
        );
        let mut permuted_pops = child.clone();
        permuted_pops
            .finality_artifact
            .validator_set_pops
            .swap(0, 1);
        let mut verifier = BridgeFinalityVerifier::with_context(network_id, anchor);
        verifier.verify(&parent.proof).expect("boundary parent");
        assert_eq!(
            verifier.verify(&permuted_pops),
            Err(BridgeFinalityVerifyError::SuccessorContextMismatch)
        );
        let mut old_key_signature = child;
        resign_v2_proof(&mut old_key_signature, &parent.keys);
        let mut verifier = BridgeFinalityVerifier::with_context(network_id, anchor);
        verifier.verify(&parent.proof).expect("boundary parent");
        assert!(matches!(
            verifier.verify(&old_key_signature),
            Err(BridgeFinalityVerifyError::CertificateVerification(
                V2QuorumCertificateVerificationError::InvalidAggregateSignature
            ))
        ));
    }
    #[test]
    fn verifier_enforces_equal_vote_npos_quorum_and_context() {
        use crate::block::consensus_v2::{
            ValidationError,
            finality::{V2FinalityValidationError, V2QuorumCertificateVerificationError},
        };
        let too_few = make_v2_fixture_with_quorum("chain-a", &[1, 1, 1, 1], &[0, 1]);
        let err = too_few
            .proof
            .finality_artifact
            .verify()
            .expect_err("two validators cannot satisfy a three-vote quorum");
        assert!(matches!(
            err,
            V2QuorumCertificateVerificationError::InvalidArtifact(
                V2FinalityValidationError::InvalidCommitCertificate(
                    ValidationError::SignerCountMismatch {
                        expected: 3,
                        actual: 2,
                    }
                )
            )
        ));
        let weighted = make_v2_fixture_with_quorum("chain-a", &[70, 10, 10, 10], &[0, 1, 2]);
        let err = weighted
            .proof
            .finality_artifact
            .verify()
            .expect_err("protocol v4 rejects weighted consensus votes");
        assert!(matches!(
            err,
            V2QuorumCertificateVerificationError::InvalidArtifact(
                V2FinalityValidationError::InvalidHeightContext(ValidationError::VotingPowerNotOne)
            )
        ));
    }
    #[test]
    fn verifier_rejects_duplicate_unsorted_and_out_of_range_signers() {
        use crate::block::consensus_v2::{
            ValidationError,
            finality::{V2FinalityValidationError, V2QuorumCertificateVerificationError},
        };
        for signers in [vec![0, 0, 2], vec![1, 0, 2], vec![0, 1, 9]] {
            let mut fixture = make_v2_fixture("chain-a");
            fixture.proof.finality_artifact.commit_qc.signers = signers;
            let err = fixture
                .proof
                .finality_artifact
                .verify()
                .expect_err("malformed signer sequence must fail before BLS");
            assert!(matches!(
                err,
                V2QuorumCertificateVerificationError::InvalidArtifact(
                    V2FinalityValidationError::InvalidCommitCertificate(
                        ValidationError::SignersNotStrictlySorted
                            | ValidationError::SignerOutOfRange
                    )
                )
            ));
        }
    }
    #[test]
    fn verifier_rejects_wrong_trusted_context() {
        let fixture = make_v2_fixture("chain-a");
        let other = make_v2_fixture("chain-a");
        let mut verifier = BridgeFinalityVerifier::with_context(
            fixture.proof.finality_artifact.height_context.network_id,
            other.proof.finality_artifact.context_id(),
        );
        assert!(matches!(
            verifier.verify(&fixture.proof),
            Err(BridgeFinalityVerifyError::UnexpectedContext { .. })
        ));
    }
}
