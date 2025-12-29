//! Governance DAG node schemas used for audit publishing.

use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use crate::{
    capacity::ReplicationOrderV1,
    deal::DealSettlementV1,
    por::{AuditVerdictV1, PorChallengeV1, PorProofV1},
};

/// Current governance log schema version.
pub const GOVERNANCE_LOG_VERSION_V1: u8 = 1;

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
        }
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
    /// Publisher signature covering the serialized node.
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deal::{
        DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
        DealSettlementStatusV1, DealSettlementV1, XorAmount,
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
}
