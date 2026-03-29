//! Domain endorsement payloads and records.

use iroha_crypto::{Hash, HashOf, PublicKey, Signature};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{domain::DomainId, nexus::DataSpaceId, prelude::Metadata};

/// Current domain endorsement version.
pub const DOMAIN_ENDORSEMENT_VERSION_V1: u8 = 1;

/// Scope covered by a domain endorsement (dataspace and optional block window).
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Encode, Decode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DomainEndorsementScope {
    /// Optional dataspace the endorsement applies to.
    pub dataspace: Option<DataSpaceId>,
    /// Optional starting block height (inclusive).
    pub block_start: Option<u64>,
    /// Optional ending block height (inclusive).
    pub block_end: Option<u64>,
}

impl DomainEndorsementScope {
    /// Check whether a block height is covered by this scope.
    #[must_use]
    pub fn contains_height(&self, height: u64) -> bool {
        if let Some(start) = self.block_start
            && height < start
        {
            return false;
        }
        if let Some(end) = self.block_end
            && height > end
        {
            return false;
        }
        true
    }
}

/// Signature over a domain endorsement statement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DomainEndorsementSignature {
    /// Signer's public key (must belong to the configured committee).
    pub signer: PublicKey,
    /// Signature over the statement hash bytes.
    pub signature: Signature,
}

/// Canonical domain endorsement body covered by signatures.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DomainEndorsement {
    /// Version byte for forward evolution.
    pub version: u8,
    /// Domain identifier being endorsed (canonical form).
    pub domain_id: DomainId,
    /// Committee identifier (opaque string for operator dashboards).
    pub committee_id: String,
    /// Hash of the canonical statement being endorsed (domain id bytes).
    pub statement_hash: Hash,
    /// Height at which the endorsement was issued.
    pub issued_at_height: u64,
    /// Height after which the endorsement is considered expired.
    pub expires_at_height: u64,
    /// Optional scope (dataspace/block window) for the endorsed statement.
    pub scope: DomainEndorsementScope,
    /// Signatures from committee members.
    pub signatures: Vec<DomainEndorsementSignature>,
    /// Optional metadata for audit trails (e.g., proposal ids).
    pub metadata: Metadata,
}

impl DomainEndorsement {
    /// Compute a hash of the endorsement body (excluding signatures).
    #[must_use]
    pub fn body_hash(&self) -> HashOf<DomainEndorsement> {
        let mut body = self.clone();
        body.signatures.clear();
        HashOf::new(&body)
    }
}

/// Committee configuration for a protected domain.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DomainCommittee {
    /// Stable committee identifier.
    pub committee_id: String,
    /// Public keys eligible to sign domain endorsements.
    pub members: Vec<PublicKey>,
    /// Minimum signatures required for acceptance.
    pub quorum: u16,
    /// Optional metadata for dashboards.
    pub metadata: Metadata,
}

impl DomainCommittee {
    /// Validate committee shape (non-empty members and sensible quorum).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let members = u16::try_from(self.members.len()).unwrap_or(u16::MAX);
        !self.members.is_empty() && self.quorum > 0 && self.quorum <= members
    }
}

/// Endorsement policy bound to a domain.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DomainEndorsementPolicy {
    /// Committee identifier this domain trusts.
    pub committee_id: String,
    /// Maximum age (in blocks) allowed between issuance and acceptance.
    pub max_endorsement_age: u64,
    /// Whether an endorsement is required for the domain.
    pub required: bool,
}

/// Stored endorsement entry used to detect replay and stale use.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DomainEndorsementRecord {
    /// Accepted endorsement payload.
    pub endorsement: DomainEndorsement,
    /// Block height when the endorsement was accepted.
    pub accepted_at_height: u64,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::KeyPair;

    use super::*;
    use crate::{metadata::Metadata, name::Name};

    #[test]
    fn scope_height_checks_bounds() {
        let scope = DomainEndorsementScope {
            dataspace: None,
            block_start: Some(10),
            block_end: Some(20),
        };
        assert!(!scope.contains_height(9));
        assert!(scope.contains_height(10));
        assert!(scope.contains_height(15));
        assert!(scope.contains_height(20));
        assert!(!scope.contains_height(21));
    }

    #[test]
    fn body_hash_stable_without_signatures() {
        let kp = KeyPair::random();
        let mut endorsement = DomainEndorsement {
            version: DOMAIN_ENDORSEMENT_VERSION_V1,
            domain_id: DomainId::new(Name::from_str("wonderland").expect("name")),
            committee_id: "default".to_owned(),
            statement_hash: Hash::prehashed([0xAA; 32]),
            issued_at_height: 10,
            expires_at_height: 20,
            scope: DomainEndorsementScope::default(),
            signatures: Vec::new(),
            metadata: Metadata::default(),
        };
        let baseline = endorsement.body_hash();
        endorsement.signatures.push(DomainEndorsementSignature {
            signer: kp.public_key().clone(),
            signature: iroha_crypto::Signature::new(kp.private_key(), baseline.as_ref()),
        });
        let with_sig = endorsement.body_hash();
        assert_eq!(baseline, with_sig);
    }

    #[test]
    fn committee_validation_rejects_bad_quorum() {
        let kp = KeyPair::random();
        let committee = DomainCommittee {
            committee_id: "c1".to_owned(),
            members: vec![kp.public_key().clone()],
            quorum: 2,
            metadata: Metadata::default(),
        };
        assert!(!committee.is_valid());
    }
}
