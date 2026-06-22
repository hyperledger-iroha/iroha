//! Governance DAG node schemas used for audit publishing.

use std::collections::{BTreeMap, BTreeSet};

use blake3::Hasher;
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signature as DalekSignature, Verifier, VerifyingKey,
};
use iroha_crypto::{Algorithm, PublicKey, Signature as IrohaSignature};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use crate::{
    capacity::ReplicationOrderV1,
    deal::DealSettlementV1,
    por::{AuditVerdictV1, PorChallengeV1, PorProofV1},
    reputation::ReputationSnapshotV1,
};

/// Current governance log schema version.
pub const GOVERNANCE_LOG_VERSION_V1: u8 = 1;

/// Current public Governance DAG block schema version.
pub const GOVERNANCE_DAG_BLOCK_VERSION_V1: u8 = 1;

/// Current public Governance DAG head manifest schema version.
pub const GOVERNANCE_DAG_HEAD_VERSION_V1: u8 = 1;

const GOVERNANCE_DAG_BLOCK_CID_DOMAIN_V1: &[u8] = b"sorafs.governance_dag.block.cid.v1";
const GOVERNANCE_LOG_NODE_CID_DOMAIN_V1: &[u8] = b"sorafs.governance_log.node.cid.v1";

/// Governance log node payload enumeration.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub enum GovernanceLogPayloadV1 {
    /// Provider advertisement snapshot.
    ProviderAdvert(crate::provider_advert::ProviderAdvertV1),
    /// Replication order snapshot.
    ReplicationOrder(ReplicationOrderV1),
    /// Proof-of-Retrievability challenge.
    PorChallenge(PorChallengeV1),
    /// Proof-of-Retrievability response.
    PorProof(PorProofV1),
    /// Audit verdict for a challenge.
    AuditVerdict(AuditVerdictV1),
    /// Deal settlement snapshot.
    DealSettlement(DealSettlementV1),
    /// Provider reputation snapshot.
    ReputationSnapshot(ReputationSnapshotV1),
}

impl GovernanceLogPayloadV1 {
    fn validate(&self, timestamp: u64) -> Result<(), GovernanceLogValidationError> {
        match self {
            GovernanceLogPayloadV1::ProviderAdvert(advert) => {
                advert
                    .validate_with_body(timestamp)
                    .map_err(GovernanceLogValidationError::Advert)?;
                Ok(())
            }
            GovernanceLogPayloadV1::ReplicationOrder(order) => order
                .validate()
                .map_err(GovernanceLogValidationError::ReplicationOrder),
            GovernanceLogPayloadV1::PorChallenge(challenge) => challenge
                .validate()
                .map_err(GovernanceLogValidationError::PorChallenge),
            GovernanceLogPayloadV1::PorProof(proof) => proof
                .validate()
                .map_err(GovernanceLogValidationError::PorProof),
            GovernanceLogPayloadV1::AuditVerdict(verdict) => verdict
                .validate()
                .map_err(GovernanceLogValidationError::AuditVerdict),
            GovernanceLogPayloadV1::DealSettlement(settlement) => settlement
                .validate()
                .map_err(GovernanceLogValidationError::DealSettlement),
            GovernanceLogPayloadV1::ReputationSnapshot(snapshot) => snapshot
                .validate()
                .map_err(GovernanceLogValidationError::ReputationSnapshot),
        }
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceLogNodeCidPayloadV1 {
    version: u8,
    prev_cid: Option<Vec<u8>>,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    payload: GovernanceLogPayloadV1,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceDagBlockCidPayloadV1 {
    version: u8,
    prev_block_cid: Option<Vec<u8>>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    node: GovernanceLogNodeV1,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceDagBlockSignaturePayloadV1 {
    version: u8,
    block_cid: Vec<u8>,
    prev_block_cid: Option<Vec<u8>>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    node: GovernanceLogNodeV1,
}

impl From<&GovernanceDagBlockV1> for GovernanceDagBlockSignaturePayloadV1 {
    fn from(block: &GovernanceDagBlockV1) -> Self {
        Self {
            version: block.version,
            block_cid: block.block_cid.clone(),
            prev_block_cid: block.prev_block_cid.clone(),
            sequence: block.sequence,
            timestamp: block.timestamp,
            publisher_peer_id: block.publisher_peer_id.clone(),
            node: block.node.clone(),
        }
    }
}

/// Public Governance DAG block wrapping one validated governance log node.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct GovernanceDagBlockV1 {
    /// Schema version (`GOVERNANCE_DAG_BLOCK_VERSION_V1`).
    pub version: u8,
    /// Deterministic BLAKE3-256 CID bytes derived from the canonical block
    /// payload excluding signatures.
    pub block_cid: Vec<u8>,
    /// Optional parent block CID.
    #[norito(default)]
    pub prev_block_cid: Option<Vec<u8>>,
    /// Monotonic sequence number in the public DAG chain.
    pub sequence: u64,
    /// Unix timestamp (seconds) when this block was assembled.
    pub timestamp: u64,
    /// Publisher peer identifier for the DAG builder/publisher.
    pub publisher_peer_id: Vec<u8>,
    /// Governance log node carried by this block.
    pub node: GovernanceLogNodeV1,
    /// Publisher signature over the canonical block signing payload.
    pub block_signature: GovernanceLogSignatureV1,
}

impl GovernanceDagBlockV1 {
    /// Returns canonical Norito bytes signed by the block publisher.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(&GovernanceDagBlockSignaturePayloadV1::from(self))
    }

    /// Recomputes this block's deterministic CID bytes.
    pub fn recompute_block_cid(&self) -> Result<Vec<u8>, norito::core::Error> {
        governance_dag_block_cid_v1(
            self.prev_block_cid.as_deref(),
            self.sequence,
            self.timestamp,
            &self.publisher_peer_id,
            &self.node,
        )
    }

    /// Validates the block structure, embedded node, CID, and block signature.
    pub fn validate(&self) -> Result<(), GovernanceDagBlockValidationError> {
        if self.version != GOVERNANCE_DAG_BLOCK_VERSION_V1 {
            return Err(GovernanceDagBlockValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.block_cid.is_empty() {
            return Err(GovernanceDagBlockValidationError::MissingBlockCid);
        }
        if self
            .prev_block_cid
            .as_ref()
            .is_some_and(|prev| prev.is_empty())
        {
            return Err(GovernanceDagBlockValidationError::InvalidPrevBlockCid);
        }
        if self.sequence == 0 && self.prev_block_cid.is_some() {
            return Err(GovernanceDagBlockValidationError::RootHasParent);
        }
        if self.sequence > 0 && self.prev_block_cid.is_none() {
            return Err(GovernanceDagBlockValidationError::NonRootMissingParent);
        }
        if self.publisher_peer_id.is_empty() {
            return Err(GovernanceDagBlockValidationError::MissingPublisherPeerId);
        }
        self.block_signature
            .validate()
            .map_err(|_| GovernanceDagBlockValidationError::InvalidSignature)?;
        self.node
            .validate()
            .map_err(GovernanceDagBlockValidationError::Node)?;
        self.node
            .verify_publisher_signature()
            .map_err(GovernanceDagBlockValidationError::NodeSignature)?;

        let expected_cid = self.recompute_block_cid().map_err(|err| {
            GovernanceDagBlockValidationError::CidEncoding {
                reason: err.to_string(),
            }
        })?;
        if self.block_cid != expected_cid {
            return Err(GovernanceDagBlockValidationError::InvalidBlockCid);
        }

        self.verify_block_signature()
            .map_err(GovernanceDagBlockValidationError::BlockSignature)
    }

    /// Verifies the block publisher signature.
    pub fn verify_block_signature(&self) -> Result<(), GovernanceLogSignatureVerificationError> {
        let payload_bytes = self.signature_payload_bytes().map_err(|err| {
            GovernanceLogSignatureVerificationError::PayloadEncoding {
                reason: err.to_string(),
            }
        })?;
        verify_governance_signature_bytes(&self.block_signature, &payload_bytes)
    }
}

/// Derives deterministic Governance DAG block CID bytes.
pub fn governance_dag_block_cid_v1(
    prev_block_cid: Option<&[u8]>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: &[u8],
    node: &GovernanceLogNodeV1,
) -> Result<Vec<u8>, norito::core::Error> {
    let payload = GovernanceDagBlockCidPayloadV1 {
        version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
        prev_block_cid: prev_block_cid.map(<[u8]>::to_vec),
        sequence,
        timestamp,
        publisher_peer_id: publisher_peer_id.to_vec(),
        node: node.clone(),
    };
    let payload_bytes = norito::to_bytes(&payload)?;
    let mut hasher = Hasher::new();
    hasher.update(GOVERNANCE_DAG_BLOCK_CID_DOMAIN_V1);
    hasher.update(&payload_bytes);
    Ok(hasher.finalize().as_bytes().to_vec())
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceDagHeadSignaturePayloadV1 {
    version: u8,
    head_block_cid: Vec<u8>,
    block_count: u64,
    generated_at: u64,
    publisher_peer_id: Vec<u8>,
    checkpoint_cid: Option<Vec<u8>>,
}

impl From<&GovernanceDagHeadV1> for GovernanceDagHeadSignaturePayloadV1 {
    fn from(head: &GovernanceDagHeadV1) -> Self {
        Self {
            version: head.version,
            head_block_cid: head.head_block_cid.clone(),
            block_count: head.block_count,
            generated_at: head.generated_at,
            publisher_peer_id: head.publisher_peer_id.clone(),
            checkpoint_cid: head.checkpoint_cid.clone(),
        }
    }
}

/// Signed public Governance DAG head manifest.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct GovernanceDagHeadV1 {
    /// Schema version (`GOVERNANCE_DAG_HEAD_VERSION_V1`).
    pub version: u8,
    /// Current head block CID.
    pub head_block_cid: Vec<u8>,
    /// Number of blocks in the chain this head advertises.
    pub block_count: u64,
    /// Unix timestamp (seconds) when this head manifest was generated.
    pub generated_at: u64,
    /// Publisher peer identifier for the head signer.
    pub publisher_peer_id: Vec<u8>,
    /// Optional trusted checkpoint or previous public head CID.
    #[norito(default)]
    pub checkpoint_cid: Option<Vec<u8>>,
    /// Publisher signature over the canonical head manifest payload.
    pub head_signature: GovernanceLogSignatureV1,
}

impl GovernanceDagHeadV1 {
    /// Returns canonical Norito bytes signed by the head publisher.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(&GovernanceDagHeadSignaturePayloadV1::from(self))
    }

    /// Validates the head manifest structure and signature.
    pub fn validate(&self) -> Result<(), GovernanceDagHeadValidationError> {
        if self.version != GOVERNANCE_DAG_HEAD_VERSION_V1 {
            return Err(GovernanceDagHeadValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.head_block_cid.is_empty() {
            return Err(GovernanceDagHeadValidationError::MissingHeadBlockCid);
        }
        if self.block_count == 0 {
            return Err(GovernanceDagHeadValidationError::EmptyBlockCount);
        }
        if self.publisher_peer_id.is_empty() {
            return Err(GovernanceDagHeadValidationError::MissingPublisherPeerId);
        }
        if self
            .checkpoint_cid
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.is_empty())
        {
            return Err(GovernanceDagHeadValidationError::InvalidCheckpointCid);
        }
        self.head_signature
            .validate()
            .map_err(|_| GovernanceDagHeadValidationError::InvalidSignature)?;
        self.verify_head_signature()
            .map_err(GovernanceDagHeadValidationError::HeadSignature)
    }

    /// Verifies the head publisher signature.
    pub fn verify_head_signature(&self) -> Result<(), GovernanceLogSignatureVerificationError> {
        let payload_bytes = self.signature_payload_bytes().map_err(|err| {
            GovernanceLogSignatureVerificationError::PayloadEncoding {
                reason: err.to_string(),
            }
        })?;
        verify_governance_signature_bytes(&self.head_signature, &payload_bytes)
    }
}

/// Signature covering a governance log node.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct GovernanceLogSignatureV1 {
    /// Signature algorithm.
    pub algorithm: GovernanceSignatureAlgorithm,
    /// Publisher public key.
    pub public_key: Vec<u8>,
    /// Raw signature bytes.
    pub signature: Vec<u8>,
}

impl GovernanceLogSignatureV1 {
    fn validate(&self) -> Result<(), GovernanceLogValidationError> {
        if self.public_key.is_empty() || self.signature.is_empty() {
            return Err(GovernanceLogValidationError::InvalidSignature);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct GovernanceLogSignaturePayloadV1 {
    version: u8,
    node_cid: Vec<u8>,
    prev_cid: Option<Vec<u8>>,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    payload: GovernanceLogPayloadV1,
}

impl From<&GovernanceLogNodeV1> for GovernanceLogSignaturePayloadV1 {
    fn from(node: &GovernanceLogNodeV1) -> Self {
        Self {
            version: node.version,
            node_cid: node.node_cid.clone(),
            prev_cid: node.prev_cid.clone(),
            timestamp: node.timestamp,
            publisher_peer_id: node.publisher_peer_id.clone(),
            payload: node.payload.clone(),
        }
    }
}

/// Algorithms supported for governance signatures.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum GovernanceSignatureAlgorithm {
    /// Ed25519 signature.
    Ed25519 = 1,
    /// Dilithium3 (post-quantum) signature.
    Dilithium3 = 2,
}

/// Governance log node entry appended to the DAG.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct GovernanceLogNodeV1 {
    /// Schema version (`GOVERNANCE_LOG_VERSION_V1`).
    pub version: u8,
    /// CID of this node (multihash bytes).
    pub node_cid: Vec<u8>,
    /// Optional previous CID in the chain.
    #[norito(default)]
    pub prev_cid: Option<Vec<u8>>,
    /// Unix timestamp (seconds) when this node was published.
    pub timestamp: u64,
    /// Publisher peer identifier (e.g., libp2p peer ID).
    pub publisher_peer_id: Vec<u8>,
    /// Payload carried by this node.
    pub payload: GovernanceLogPayloadV1,
    /// Publisher signature covering the canonical node signing payload.
    pub publisher_signature: GovernanceLogSignatureV1,
}

impl GovernanceLogNodeV1 {
    /// Validates the log node payload.
    pub fn validate(&self) -> Result<(), GovernanceLogValidationError> {
        if self.version != GOVERNANCE_LOG_VERSION_V1 {
            return Err(GovernanceLogValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.node_cid.is_empty() {
            return Err(GovernanceLogValidationError::MissingNodeCid);
        }
        if self.prev_cid.as_ref().is_some_and(|prev| prev.is_empty()) {
            return Err(GovernanceLogValidationError::InvalidPrevCid);
        }
        if self.publisher_peer_id.is_empty() {
            return Err(GovernanceLogValidationError::MissingPublisherPeerId);
        }
        self.publisher_signature.validate()?;
        self.payload.validate(self.timestamp)?;
        Ok(())
    }

    /// Returns canonical Norito bytes signed by the publisher.
    ///
    /// The payload deliberately excludes `publisher_signature` so signers and
    /// verifiers use stable bytes before and after the signature is attached.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(&GovernanceLogSignaturePayloadV1::from(self))
    }

    /// Recomputes this node's deterministic CID bytes.
    pub fn recompute_node_cid(&self) -> Result<Vec<u8>, norito::core::Error> {
        governance_log_node_cid_v1(
            self.prev_cid.as_deref(),
            self.timestamp,
            &self.publisher_peer_id,
            &self.payload,
        )
    }

    /// Verifies a publisher signature over the canonical node payload.
    pub fn verify_publisher_signature(
        &self,
    ) -> Result<(), GovernanceLogSignatureVerificationError> {
        let payload_bytes = self.signature_payload_bytes().map_err(|err| {
            GovernanceLogSignatureVerificationError::PayloadEncoding {
                reason: err.to_string(),
            }
        })?;

        verify_governance_signature_bytes(&self.publisher_signature, &payload_bytes)
    }
}

/// Derives deterministic Governance log node CID bytes.
pub fn governance_log_node_cid_v1(
    prev_cid: Option<&[u8]>,
    timestamp: u64,
    publisher_peer_id: &[u8],
    payload: &GovernanceLogPayloadV1,
) -> Result<Vec<u8>, norito::core::Error> {
    let payload = GovernanceLogNodeCidPayloadV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        prev_cid: prev_cid.map(<[u8]>::to_vec),
        timestamp,
        publisher_peer_id: publisher_peer_id.to_vec(),
        payload: payload.clone(),
    };
    let payload_bytes = norito::to_bytes(&payload)?;
    let mut hasher = Hasher::new();
    hasher.update(GOVERNANCE_LOG_NODE_CID_DOMAIN_V1);
    hasher.update(&payload_bytes);
    Ok(hasher.finalize().as_bytes().to_vec())
}

fn verify_governance_signature_bytes(
    publisher_signature: &GovernanceLogSignatureV1,
    payload_bytes: &[u8],
) -> Result<(), GovernanceLogSignatureVerificationError> {
    match publisher_signature.algorithm {
        GovernanceSignatureAlgorithm::Ed25519 => {
            verify_ed25519_governance_signature(publisher_signature, payload_bytes)
        }
        GovernanceSignatureAlgorithm::Dilithium3 => {
            verify_mldsa_governance_signature(publisher_signature, payload_bytes)
        }
    }
}

fn verify_ed25519_governance_signature(
    publisher_signature: &GovernanceLogSignatureV1,
    payload_bytes: &[u8],
) -> Result<(), GovernanceLogSignatureVerificationError> {
    if publisher_signature.public_key.len() != PUBLIC_KEY_LENGTH {
        return Err(
            GovernanceLogSignatureVerificationError::InvalidPublicKeyLength {
                length: publisher_signature.public_key.len(),
            },
        );
    }
    if publisher_signature.signature.len() != SIGNATURE_LENGTH {
        return Err(
            GovernanceLogSignatureVerificationError::InvalidSignatureLength {
                length: publisher_signature.signature.len(),
            },
        );
    }

    let mut public_key = [0u8; PUBLIC_KEY_LENGTH];
    public_key.copy_from_slice(&publisher_signature.public_key);
    let verifying_key = VerifyingKey::from_bytes(&public_key).map_err(|err| {
        GovernanceLogSignatureVerificationError::InvalidPublicKey {
            reason: err.to_string(),
        }
    })?;

    let mut signature = [0u8; SIGNATURE_LENGTH];
    signature.copy_from_slice(&publisher_signature.signature);
    let signature = DalekSignature::from_bytes(&signature);

    verifying_key
        .verify(payload_bytes, &signature)
        .map_err(
            |err| GovernanceLogSignatureVerificationError::Verification {
                reason: err.to_string(),
            },
        )
}

fn verify_mldsa_governance_signature(
    publisher_signature: &GovernanceLogSignatureV1,
    payload_bytes: &[u8],
) -> Result<(), GovernanceLogSignatureVerificationError> {
    let public_key = PublicKey::from_bytes(Algorithm::MlDsa, &publisher_signature.public_key)
        .map_err(
            |err| GovernanceLogSignatureVerificationError::InvalidPublicKey {
                reason: err.to_string(),
            },
        )?;
    let signature = IrohaSignature::from_bytes(&publisher_signature.signature);
    signature.verify(&public_key, payload_bytes).map_err(|err| {
        GovernanceLogSignatureVerificationError::Verification {
            reason: err.to_string(),
        }
    })
}

/// Validation errors for governance log nodes.
#[derive(Debug, Error)]
pub enum GovernanceLogValidationError {
    #[error("unsupported governance log version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("node CID must not be empty")]
    MissingNodeCid,
    #[error("previous CID must be None or non-empty")]
    InvalidPrevCid,
    #[error("publisher peer ID must not be empty")]
    MissingPublisherPeerId,
    #[error("publisher signature missing key or signature bytes")]
    InvalidSignature,
    #[error("advert validation failed: {0}")]
    Advert(crate::provider_advert::AdvertValidationError),
    #[error("replication order validation failed: {0}")]
    ReplicationOrder(crate::capacity::ReplicationOrderValidationError),
    #[error("challenge validation failed: {0}")]
    PorChallenge(crate::por::PorChallengeValidationError),
    #[error("proof validation failed: {0}")]
    PorProof(crate::por::PorProofValidationError),
    #[error("audit verdict validation failed: {0}")]
    AuditVerdict(crate::por::AuditVerdictValidationError),
    #[error("deal settlement validation failed: {0}")]
    DealSettlement(crate::deal::DealSettlementValidationError),
    #[error("reputation snapshot validation failed: {0}")]
    ReputationSnapshot(crate::reputation::ReputationValidationError),
}

/// Validation errors for public Governance DAG blocks.
#[derive(Debug, Error)]
pub enum GovernanceDagBlockValidationError {
    #[error("unsupported governance DAG block version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("block CID must not be empty")]
    MissingBlockCid,
    #[error("previous block CID must be None or non-empty")]
    InvalidPrevBlockCid,
    #[error("root governance DAG block must not carry a previous block CID")]
    RootHasParent,
    #[error("non-root governance DAG block must carry a previous block CID")]
    NonRootMissingParent,
    #[error("publisher peer ID must not be empty")]
    MissingPublisherPeerId,
    #[error("block signature missing key or signature bytes")]
    InvalidSignature,
    #[error("embedded governance node validation failed: {0}")]
    Node(GovernanceLogValidationError),
    #[error("embedded governance node signature validation failed: {0}")]
    NodeSignature(GovernanceLogSignatureVerificationError),
    #[error("failed to encode governance DAG block CID payload: {reason}")]
    CidEncoding { reason: String },
    #[error("governance DAG block CID does not match the canonical block payload")]
    InvalidBlockCid,
    #[error("governance DAG block signature validation failed: {0}")]
    BlockSignature(GovernanceLogSignatureVerificationError),
}

/// Validation errors for public Governance DAG head manifests.
#[derive(Debug, Error)]
pub enum GovernanceDagHeadValidationError {
    #[error("unsupported governance DAG head version {found}")]
    UnsupportedVersion { found: u8 },
    #[error("head block CID must not be empty")]
    MissingHeadBlockCid,
    #[error("head manifest block count must be greater than zero")]
    EmptyBlockCount,
    #[error("publisher peer ID must not be empty")]
    MissingPublisherPeerId,
    #[error("checkpoint CID must be None or non-empty")]
    InvalidCheckpointCid,
    #[error("head signature missing key or signature bytes")]
    InvalidSignature,
    #[error("governance DAG head signature validation failed: {0}")]
    HeadSignature(GovernanceLogSignatureVerificationError),
}

/// Validation errors for Governance DAG block chains.
#[derive(Debug, Error)]
pub enum GovernanceDagChainValidationError {
    #[error("governance DAG chain must contain at least one block")]
    Empty,
    #[error("block at index {index} failed validation: {source}")]
    InvalidBlock {
        index: usize,
        source: GovernanceDagBlockValidationError,
    },
    #[error("duplicate governance DAG block CID at index {index}")]
    DuplicateBlockCid { index: usize },
    #[error("block at index {index} references a missing parent")]
    MissingParent { index: usize },
    #[error("block at index {index} has sequence {sequence}, expected {expected}")]
    SequenceGap {
        index: usize,
        expected: u64,
        sequence: u64,
    },
    #[error("block at index {index} has timestamp earlier than its parent")]
    TimestampRegression { index: usize },
    #[error("expected exactly one governance DAG head, found {count}")]
    HeadCount { count: usize },
    #[error("governance DAG head does not match expected CID")]
    ExpectedHeadMismatch,
}

/// Validation errors for binding a signed head manifest to a block chain.
#[derive(Debug, Error)]
pub enum GovernanceDagHeadChainValidationError {
    #[error("head manifest validation failed: {0}")]
    Head(GovernanceDagHeadValidationError),
    #[error("block chain validation failed: {0}")]
    Chain(GovernanceDagChainValidationError),
    #[error("head block count {head_count} does not match chain block count {chain_count}")]
    BlockCountMismatch { head_count: u64, chain_count: u64 },
}

/// Validates a public Governance DAG chain and optional expected head CID.
pub fn validate_governance_dag_chain_v1(
    blocks: &[GovernanceDagBlockV1],
    expected_head_cid: Option<&[u8]>,
) -> Result<(), GovernanceDagChainValidationError> {
    if blocks.is_empty() {
        return Err(GovernanceDagChainValidationError::Empty);
    }

    let mut block_by_cid = BTreeMap::<Vec<u8>, usize>::new();
    let mut referenced_parents = BTreeSet::<Vec<u8>>::new();
    for (index, block) in blocks.iter().enumerate() {
        block
            .validate()
            .map_err(|source| GovernanceDagChainValidationError::InvalidBlock { index, source })?;
        if block_by_cid
            .insert(block.block_cid.clone(), index)
            .is_some()
        {
            return Err(GovernanceDagChainValidationError::DuplicateBlockCid { index });
        }
        if let Some(prev) = &block.prev_block_cid {
            referenced_parents.insert(prev.clone());
        }
    }

    for (index, block) in blocks.iter().enumerate() {
        let Some(prev) = &block.prev_block_cid else {
            continue;
        };
        let Some(parent_index) = block_by_cid.get(prev).copied() else {
            return Err(GovernanceDagChainValidationError::MissingParent { index });
        };
        let parent = &blocks[parent_index];
        let expected = parent.sequence.saturating_add(1);
        if block.sequence != expected {
            return Err(GovernanceDagChainValidationError::SequenceGap {
                index,
                expected,
                sequence: block.sequence,
            });
        }
        if block.timestamp < parent.timestamp {
            return Err(GovernanceDagChainValidationError::TimestampRegression { index });
        }
    }

    let mut heads = Vec::<&[u8]>::new();
    for block in blocks {
        if !referenced_parents.contains(&block.block_cid) {
            heads.push(block.block_cid.as_slice());
        }
    }
    if heads.len() != 1 {
        return Err(GovernanceDagChainValidationError::HeadCount { count: heads.len() });
    }
    if let Some(expected_head_cid) = expected_head_cid
        && heads[0] != expected_head_cid
    {
        return Err(GovernanceDagChainValidationError::ExpectedHeadMismatch);
    }
    Ok(())
}

/// Validates a signed head manifest against its advertised block chain.
pub fn validate_governance_dag_head_against_chain_v1(
    head: &GovernanceDagHeadV1,
    blocks: &[GovernanceDagBlockV1],
) -> Result<(), GovernanceDagHeadChainValidationError> {
    head.validate()
        .map_err(GovernanceDagHeadChainValidationError::Head)?;
    validate_governance_dag_chain_v1(blocks, Some(&head.head_block_cid))
        .map_err(GovernanceDagHeadChainValidationError::Chain)?;
    let chain_count = u64::try_from(blocks.len()).unwrap_or(u64::MAX);
    if head.block_count != chain_count {
        return Err(GovernanceDagHeadChainValidationError::BlockCountMismatch {
            head_count: head.block_count,
            chain_count,
        });
    }
    Ok(())
}

/// Errors raised while verifying a governance log publisher signature.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GovernanceLogSignatureVerificationError {
    /// Signature algorithm is not supported by this validator.
    #[error("unsupported governance log signature algorithm: {0:?}")]
    UnsupportedAlgorithm(GovernanceSignatureAlgorithm),
    /// Ed25519 public key length is invalid.
    #[error("ed25519 governance public key must be 32 bytes, got {length}")]
    InvalidPublicKeyLength {
        /// Observed public key byte length.
        length: usize,
    },
    /// Ed25519 signature length is invalid.
    #[error("ed25519 governance signature must be 64 bytes, got {length}")]
    InvalidSignatureLength {
        /// Observed signature byte length.
        length: usize,
    },
    /// Public key bytes could not be parsed.
    #[error("invalid governance public key: {reason}")]
    InvalidPublicKey {
        /// Underlying parser diagnostic.
        reason: String,
    },
    /// Canonical signature payload could not be encoded.
    #[error("failed to encode governance log signature payload: {reason}")]
    PayloadEncoding {
        /// Underlying Norito diagnostic.
        reason: String,
    },
    /// Signature verification failed.
    #[error("governance log publisher signature verification failed: {reason}")]
    Verification {
        /// Underlying signature verification diagnostic.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};

    fn signed_por_proof_payload() -> GovernanceLogPayloadV1 {
        GovernanceLogPayloadV1::PorProof(crate::por::PorProofV1 {
            version: crate::POR_PROOF_VERSION_V1,
            challenge_id: [0x11; 32],
            manifest_digest: [0x22; 32],
            provider_id: [0x33; 32],
            samples: vec![crate::por::PorProofSampleV1 {
                sample_index: 7,
                chunk_offset: 4096,
                chunk_size: 1024,
                chunk_digest: [0x44; 32],
                leaf_digest: [0x55; 32],
            }],
            auth_path: vec![[0x66; 32]],
            signature: crate::provider_advert::AdvertSignature {
                algorithm: crate::provider_advert::SignatureAlgorithm::Ed25519,
                public_key: vec![0x77; 32],
                signature: vec![0x88; 64],
            },
            submitted_at: 1_700_000_200,
        })
    }

    fn governance_node_for_signing() -> GovernanceLogNodeV1 {
        GovernanceLogNodeV1 {
            version: GOVERNANCE_LOG_VERSION_V1,
            node_cid: b"bafygovernancelognode".to_vec(),
            prev_cid: Some(b"bafypreviouscid".to_vec()),
            timestamp: 1_700_000_300,
            publisher_peer_id: b"12D3KooWGovernancePeer".to_vec(),
            payload: signed_por_proof_payload(),
            publisher_signature: GovernanceLogSignatureV1 {
                algorithm: GovernanceSignatureAlgorithm::Dilithium3,
                public_key: vec![0x99; 64],
                signature: vec![0xAA; 160],
            },
        }
    }

    #[test]
    fn governance_log_node_cid_is_stable_and_input_sensitive() {
        let payload = signed_por_proof_payload();
        let prev_cid = b"bafypreviouscid";
        let publisher_peer_id = b"12D3KooWGovernancePeer";
        let first = governance_log_node_cid_v1(
            Some(prev_cid.as_slice()),
            1_700_000_300,
            publisher_peer_id,
            &payload,
        )
        .expect("derive governance log node CID");
        let second = governance_log_node_cid_v1(
            Some(prev_cid.as_slice()),
            1_700_000_300,
            publisher_peer_id,
            &payload,
        )
        .expect("derive governance log node CID again");
        let changed = governance_log_node_cid_v1(
            Some(prev_cid.as_slice()),
            1_700_000_301,
            publisher_peer_id,
            &payload,
        )
        .expect("derive changed governance log node CID");

        assert_eq!(first, second);
        assert_ne!(first, changed);
        assert_eq!(first.len(), blake3::OUT_LEN);
    }

    fn sign_governance_node(node: &mut GovernanceLogNodeV1, seed: &[u8; 32]) {
        let signing_key = SigningKey::from_bytes(seed);
        let payload_bytes = node
            .signature_payload_bytes()
            .expect("encode governance signing payload");
        let signature = signing_key.sign(&payload_bytes);
        node.publisher_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        };
    }

    fn sign_governance_node_mldsa(node: &mut GovernanceLogNodeV1, seed: &[u8]) {
        let key_pair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::MlDsa)
            .expect("generate ML-DSA governance keypair");
        let payload_bytes = node
            .signature_payload_bytes()
            .expect("encode governance signing payload");
        let signature = IrohaSignature::try_new(key_pair.private_key(), &payload_bytes)
            .expect("sign governance payload with ML-DSA key");
        let (algorithm, public_key) = key_pair
            .public_key()
            .try_to_bytes()
            .expect("encode ML-DSA public key");
        assert_eq!(algorithm, Algorithm::MlDsa);
        node.publisher_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Dilithium3,
            public_key: public_key.to_vec(),
            signature: signature.payload().to_vec(),
        };
    }

    fn empty_ed25519_signature() -> GovernanceLogSignatureV1 {
        GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn sign_governance_block(block: &mut GovernanceDagBlockV1, seed: &[u8; 32]) {
        let signing_key = SigningKey::from_bytes(seed);
        let payload_bytes = block
            .signature_payload_bytes()
            .expect("encode governance DAG block signing payload");
        let signature = signing_key.sign(&payload_bytes);
        block.block_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        };
    }

    fn sign_governance_head(head: &mut GovernanceDagHeadV1, seed: &[u8; 32]) {
        let signing_key = SigningKey::from_bytes(seed);
        let payload_bytes = head
            .signature_payload_bytes()
            .expect("encode governance DAG head signing payload");
        let signature = signing_key.sign(&payload_bytes);
        head.head_signature = GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: signing_key.verifying_key().to_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        };
    }

    fn signed_governance_block(
        prev_block_cid: Option<Vec<u8>>,
        sequence: u64,
        timestamp: u64,
    ) -> GovernanceDagBlockV1 {
        let mut node = governance_node_for_signing();
        node.node_cid = format!("bafygovernancelognode{sequence}").into_bytes();
        node.prev_cid = sequence
            .checked_sub(1)
            .map(|prev| format!("bafygovernancelognode{prev}").into_bytes());
        node.timestamp = timestamp;
        sign_governance_node(&mut node, &[0xA5; 32]);

        let publisher_peer_id = b"12D3KooWGovernanceDagPublisher".to_vec();
        let block_cid = governance_dag_block_cid_v1(
            prev_block_cid.as_deref(),
            sequence,
            timestamp + 10,
            &publisher_peer_id,
            &node,
        )
        .expect("derive governance DAG block CID");
        let mut block = GovernanceDagBlockV1 {
            version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
            block_cid,
            prev_block_cid,
            sequence,
            timestamp: timestamp + 10,
            publisher_peer_id,
            node,
            block_signature: empty_ed25519_signature(),
        };
        sign_governance_block(&mut block, &[0xC7; 32]);
        block
    }

    fn signed_governance_head(blocks: &[GovernanceDagBlockV1]) -> GovernanceDagHeadV1 {
        let head_block_cid = blocks.last().expect("at least one block").block_cid.clone();
        let mut head = GovernanceDagHeadV1 {
            version: GOVERNANCE_DAG_HEAD_VERSION_V1,
            head_block_cid,
            block_count: blocks.len() as u64,
            generated_at: 1_700_001_000,
            publisher_peer_id: b"12D3KooWGovernanceDagPublisher".to_vec(),
            checkpoint_cid: None,
            head_signature: empty_ed25519_signature(),
        };
        sign_governance_head(&mut head, &[0xD9; 32]);
        head
    }
    use crate::deal::{
        DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
        DealSettlementStatusV1, DealSettlementV1, XorAmount,
    };
    use crate::reputation::{
        REPUTATION_PROVIDER_INPUT_VERSION_V1, REPUTATION_PROVIDER_METRICS_VERSION_V1,
        ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
        ReputationWeightsV1, build_reputation_snapshot,
    };

    #[test]
    fn governance_node_validation_succeeds() {
        let mut builder = crate::provider_advert::ProviderAdvertV1::builder();
        let range_capability = crate::provider_advert::ProviderCapabilityRangeV1 {
            max_chunk_span: 1_048_576,
            min_granularity: 4_096,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: true,
        };
        let _ = builder
            .profile_id("sorafs.sf1@1.0.0")
            .profile_aliases(vec![
                "sorafs.sf1@1.0.0".to_string(),
                "sorafs-sf1".to_string(),
            ])
            .provider_id([5; 32])
            .stake_pool_id([6; 32])
            .stake_amount(1_000_000)
            .availability(crate::provider_advert::AvailabilityTier::Hot)
            .max_retrieval_latency_ms(250)
            .max_concurrent_streams(32)
            .add_capability(crate::provider_advert::CapabilityTlv {
                cap_type: crate::provider_advert::CapabilityType::ToriiGateway,
                payload: Vec::new(),
            })
            .add_range_capability(range_capability)
            .expect("range capability")
            .add_endpoint(crate::provider_advert::AdvertEndpoint {
                kind: crate::provider_advert::EndpointKind::Torii,
                host_pattern: "gateway.sora".to_string(),
                metadata: Vec::new(),
            })
            .add_topic(crate::provider_advert::RendezvousTopic {
                topic: "sorafs.sf1.primary".to_string(),
                region: "global".to_string(),
            })
            .path_policy_min_guard_weight(5)
            .path_policy_max_same_asn_per_path(2)
            .path_policy_max_same_pool_per_path(1)
            .stream_budget(crate::provider_advert::StreamBudgetV1 {
                max_in_flight: 4,
                max_bytes_per_sec: 512_000,
                burst_bytes: Some(64_000),
            })
            .add_transport_hint(crate::provider_advert::TransportHintV1 {
                protocol: crate::provider_advert::TransportProtocol::ToriiHttpRange,
                priority: 0,
            })
            .issued_at(1_700_000_000)
            .ttl_secs(3_600);
        let _ = builder.signature(
            crate::provider_advert::SignatureAlgorithm::Ed25519,
            vec![9; 32],
            vec![10; 64],
        );
        let advert = builder.build().expect("valid advert");

        let node = GovernanceLogNodeV1 {
            version: GOVERNANCE_LOG_VERSION_V1,
            node_cid: b"bafygovernancenodecid".to_vec(),
            prev_cid: Some(b"bafypreviouscid".to_vec()),
            timestamp: 1_700_000_100,
            publisher_peer_id: b"12D3KooWGovernancePeer".to_vec(),
            payload: GovernanceLogPayloadV1::ProviderAdvert(advert),
            publisher_signature: GovernanceLogSignatureV1 {
                algorithm: GovernanceSignatureAlgorithm::Dilithium3,
                public_key: vec![11; 64],
                signature: vec![12; 160],
            },
        };

        assert!(node.validate().is_ok());
    }

    #[test]
    fn governance_signature_payload_excludes_publisher_signature() {
        let node = governance_node_for_signing();
        let mut different_signature = node.clone();
        different_signature.publisher_signature.signature = vec![0xBB; 96];

        assert_eq!(
            node.signature_payload_bytes()
                .expect("encode governance signature payload"),
            different_signature
                .signature_payload_bytes()
                .expect("encode governance signature payload")
        );

        let mut different_payload = node.clone();
        different_payload.timestamp += 1;
        assert_ne!(
            node.signature_payload_bytes()
                .expect("encode governance signature payload"),
            different_payload
                .signature_payload_bytes()
                .expect("encode governance signature payload")
        );
    }

    #[test]
    fn verify_publisher_signature_accepts_ed25519_signed_node() {
        let seed = [0xA5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node(&mut node, &seed);

        node.verify_publisher_signature()
            .expect("governance node signature verifies");
    }

    #[test]
    fn verify_publisher_signature_rejects_tampered_payload() {
        let seed = [0xA5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node(&mut node, &seed);
        node.timestamp += 1;

        assert!(matches!(
            node.verify_publisher_signature(),
            Err(GovernanceLogSignatureVerificationError::Verification { .. })
        ));
    }

    #[test]
    fn verify_publisher_signature_accepts_dilithium3_signed_node() {
        let seed = [0xB5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node_mldsa(&mut node, &seed);

        node.verify_publisher_signature()
            .expect("ML-DSA governance node signature verifies");
    }

    #[test]
    fn verify_publisher_signature_rejects_tampered_dilithium3_payload() {
        let seed = [0xB5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node_mldsa(&mut node, &seed);
        node.publisher_peer_id.extend_from_slice(b"-tampered");

        assert!(matches!(
            node.verify_publisher_signature(),
            Err(GovernanceLogSignatureVerificationError::Verification { .. })
        ));
    }

    #[test]
    fn governance_dag_block_derives_cid_and_verifies_signature() {
        let block = signed_governance_block(None, 0, 1_700_000_400);

        block.validate().expect("valid governance DAG block");
        assert_eq!(
            block
                .recompute_block_cid()
                .expect("recompute governance DAG block CID"),
            block.block_cid
        );
        block
            .verify_block_signature()
            .expect("block signature verifies");
    }

    #[test]
    fn governance_dag_block_signature_payload_excludes_signature() {
        let block = signed_governance_block(None, 0, 1_700_000_400);
        let mut different_signature = block.clone();
        different_signature.block_signature.signature = vec![0xEE; 64];

        assert_eq!(
            block
                .signature_payload_bytes()
                .expect("encode block signature payload"),
            different_signature
                .signature_payload_bytes()
                .expect("encode block signature payload")
        );

        let mut different_payload = block.clone();
        different_payload.sequence = 1;
        assert_ne!(
            block
                .signature_payload_bytes()
                .expect("encode block signature payload"),
            different_payload
                .signature_payload_bytes()
                .expect("encode block signature payload")
        );
    }

    #[test]
    fn governance_dag_block_rejects_tampered_cid() {
        let mut block = signed_governance_block(None, 0, 1_700_000_400);
        block.block_cid[0] ^= 0x01;

        assert!(matches!(
            block.validate(),
            Err(GovernanceDagBlockValidationError::InvalidBlockCid)
        ));
    }

    #[test]
    fn governance_dag_chain_validates_parent_linkage_and_head() {
        let root = signed_governance_block(None, 0, 1_700_000_400);
        let child = signed_governance_block(Some(root.block_cid.clone()), 1, 1_700_000_500);
        let blocks = vec![root, child];
        let expected_head = blocks[1].block_cid.clone();

        validate_governance_dag_chain_v1(&blocks, Some(&expected_head))
            .expect("valid governance DAG chain");
    }

    #[test]
    fn governance_dag_chain_rejects_missing_parent() {
        let block = signed_governance_block(Some(vec![0xA5; 32]), 1, 1_700_000_500);

        assert!(matches!(
            validate_governance_dag_chain_v1(&[block], None),
            Err(GovernanceDagChainValidationError::MissingParent { index: 0 })
        ));
    }

    #[test]
    fn governance_dag_head_manifest_signs_and_binds_chain() {
        let root = signed_governance_block(None, 0, 1_700_000_400);
        let child = signed_governance_block(Some(root.block_cid.clone()), 1, 1_700_000_500);
        let blocks = vec![root, child];
        let head = signed_governance_head(&blocks);

        head.validate().expect("valid governance DAG head");
        head.verify_head_signature()
            .expect("head signature verifies");
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("head binds the governance DAG chain");
    }

    #[test]
    fn governance_dag_head_rejects_block_count_mismatch() {
        let root = signed_governance_block(None, 0, 1_700_000_400);
        let blocks = vec![root];
        let mut head = signed_governance_head(&blocks);
        head.block_count += 1;
        sign_governance_head(&mut head, &[0xD9; 32]);

        assert!(matches!(
            validate_governance_dag_head_against_chain_v1(&head, &blocks),
            Err(GovernanceDagHeadChainValidationError::BlockCountMismatch {
                head_count: 2,
                chain_count: 1
            })
        ));
    }

    #[test]
    fn governance_payload_accepts_deal_settlement() {
        let ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            deal_id: [0xAA; 32],
            provider_id: [0xBB; 32],
            client_id: [0xCC; 32],
            provider_accrual: XorAmount::from_micro(100),
            client_liability: XorAmount::from_micro(100),
            bond_locked: XorAmount::from_micro(50),
            bond_slashed: XorAmount::zero(),
            captured_at: 1_700_200_000,
        };
        let settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            deal_id: [0xAA; 32],
            ledger,
            status: DealSettlementStatusV1::Completed,
            settled_at: 1_700_200_100,
            audit_notes: None,
        };
        let payload = GovernanceLogPayloadV1::DealSettlement(settlement);
        payload.validate(1_700_200_200).expect("valid settlement");
    }

    #[test]
    fn governance_payload_accepts_reputation_snapshot() {
        let input = ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: "provider-a".to_string(),
            metrics: ReputationProviderMetricsV1 {
                version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
                por_success_bps: 9_600,
                pdp_success_bps: 9_700,
                potr_success_bps: 9_500,
                latency_health_bps: 9_100,
                dispute_rate_bps: 0,
                token_violation_rate_bps: 0,
                repair_breach_rate_bps: 0,
            },
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        };
        let snapshot = build_reputation_snapshot(
            [0x42; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input],
            None,
        )
        .expect("reputation snapshot");
        let payload = GovernanceLogPayloadV1::ReputationSnapshot(snapshot);

        payload
            .validate(1_800_000_100)
            .expect("valid reputation snapshot");
    }
}
