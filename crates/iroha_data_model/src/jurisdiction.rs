//! Jurisdiction Data Guardian (JDG) attestations and committee types.
//!
//! These types define the canonical Norito encoding for JDG attestations,
//! including their scope, signer sets, optional proofs, and the domain-tagged
//! hash used for signing.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey, Signature, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{nexus::DataSpaceId, proof::ProofBox};

/// Maximum supported length (bytes) for a jurisdiction identifier.
pub const JDG_MAX_JURISDICTION_ID_LEN: usize = 64;

/// Explicit attestation format version (v1).
pub const JDG_ATTESTATION_VERSION_V1: u16 = 1;

/// Domain tag used when hashing [`JdgAttestation`] for signing.
pub const JDG_ATTESTATION_DOMAIN_TAG_V1: &[u8] = b"iroha:jurisdiction:attestation:v1\x00";

/// Explicit SDN commitment format version (v1).
pub const JDG_SDN_COMMITMENT_VERSION_V1: u16 = 1;

/// Domain tag used when hashing [`JdgSdnCommitment`] for signing.
pub const JDG_SDN_COMMITMENT_DOMAIN_TAG_V1: &[u8] = b"iroha:jurisdiction:sdn:commitment:v1\x00";

/// Identifier for a jurisdiction (e.g., derived from BIC/ClearingSystemId).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JurisdictionId(Vec<u8>);

impl JurisdictionId {
    /// Construct a jurisdiction identifier from canonical bytes.
    ///
    /// # Errors
    ///
    /// Returns [`JurisdictionIdError`] when the identifier is empty or exceeds
    /// [`JDG_MAX_JURISDICTION_ID_LEN`].
    pub fn new(bytes: Vec<u8>) -> Result<Self, JurisdictionIdError> {
        if bytes.is_empty() {
            return Err(JurisdictionIdError::Empty);
        }
        if bytes.len() > JDG_MAX_JURISDICTION_ID_LEN {
            return Err(JurisdictionIdError::TooLong {
                len: bytes.len(),
                max: JDG_MAX_JURISDICTION_ID_LEN,
            });
        }
        Ok(Self(bytes))
    }

    /// Return the canonical identifier bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl TryFrom<Vec<u8>> for JurisdictionId {
    type Error = JurisdictionIdError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<JurisdictionId> for Vec<u8> {
    fn from(value: JurisdictionId) -> Self {
        value.0
    }
}

/// Errors returned when constructing a [`JurisdictionId`].
#[derive(Debug, Copy, Clone, Error, PartialEq, Eq)]
pub enum JurisdictionIdError {
    /// Identifier contained zero bytes.
    #[error("jurisdiction id must not be empty")]
    Empty,
    /// Identifier exceeded the supported length.
    #[error("jurisdiction id length {len} exceeds maximum {max}")]
    TooLong {
        /// Provided identifier length.
        len: usize,
        /// Maximum allowed length.
        max: usize,
    },
}

/// Inclusive block-height range covered by an attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgBlockRange {
    /// First block height (inclusive).
    pub start_height: u64,
    /// Last block height (inclusive).
    pub end_height: u64,
}

impl JdgBlockRange {
    /// Construct a block range ensuring `start_height <= end_height`.
    ///
    /// # Errors
    ///
    /// Returns [`JdgAttestationError::InvalidBlockRange`] when the bounds are invalid.
    pub fn new(start_height: u64, end_height: u64) -> Result<Self, JdgAttestationError> {
        if start_height > end_height {
            return Err(JdgAttestationError::InvalidBlockRange {
                start_height,
                end_height,
            });
        }
        Ok(Self {
            start_height,
            end_height,
        })
    }

    /// Check whether `height` lies inside the range.
    #[must_use]
    pub fn contains(&self, height: u64) -> bool {
        height >= self.start_height && height <= self.end_height
    }

    /// Validate the block range bounds.
    ///
    /// # Errors
    ///
    /// Returns [`JdgAttestationError::InvalidBlockRange`] when `start_height > end_height`.
    pub fn validate(&self) -> Result<(), JdgAttestationError> {
        if self.start_height > self.end_height {
            Err(JdgAttestationError::InvalidBlockRange {
                start_height: self.start_height,
                end_height: self.end_height,
            })
        } else {
            Ok(())
        }
    }
}

/// Statement scope bound into an attestation (dataspace + block window + jurisdiction id).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgAttestationScope {
    /// Jurisdiction identifier for the attested execution.
    pub jurisdiction_id: JurisdictionId,
    /// Dataspace the attestation applies to.
    pub dataspace: DataSpaceId,
    /// Covered block range (inclusive).
    pub block_range: JdgBlockRange,
}

/// Canonical read/write access set used by the attested execution.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgStateAccessSet {
    /// Canonical read keys (byte-sorted and deduplicated).
    pub reads: Vec<Vec<u8>>,
    /// Canonical write keys (byte-sorted and deduplicated).
    pub writes: Vec<Vec<u8>>,
}

impl JdgStateAccessSet {
    /// Build a normalized access set by sorting and deduplicating the supplied keys.
    #[must_use]
    pub fn normalized(reads: Vec<Vec<u8>>, writes: Vec<Vec<u8>>) -> Self {
        Self {
            reads: normalize_keys(reads),
            writes: normalize_keys(writes),
        }
    }
}

/// Verdict emitted by a JDG after evaluating policy/rules.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "verdict", content = "value")
)]
pub enum JdgVerdict {
    /// Business rules accepted the transaction.
    Accept,
    /// Business rules rejected the transaction with a stable code.
    Reject(u32),
}

/// Threshold scheme with per-signer signatures.
pub const JDG_SIGNATURE_SCHEME_SIMPLE_THRESHOLD: u16 = 1;
/// Pre-aggregated BLS-normal signature for same-message validation.
pub const JDG_SIGNATURE_SCHEME_BLS_NORMAL_AGGREGATE: u16 = 2;

/// Allowed JDG signature scheme identifiers.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash, Default,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "scheme", content = "value"))]
pub enum JdgSignatureScheme {
    /// Per-signer signatures with optional signer bitmap.
    #[default]
    SimpleThreshold,
    /// Pre-aggregated BLS-normal signature over the attestation hash.
    BlsNormalAggregate,
}

impl JdgSignatureScheme {
    /// Returns the numeric scheme identifier.
    #[must_use]
    pub const fn scheme_id(self) -> u16 {
        match self {
            Self::SimpleThreshold => JDG_SIGNATURE_SCHEME_SIMPLE_THRESHOLD,
            Self::BlsNormalAggregate => JDG_SIGNATURE_SCHEME_BLS_NORMAL_AGGREGATE,
        }
    }

    /// Returns the canonical string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SimpleThreshold => "simple_threshold",
            Self::BlsNormalAggregate => "bls_normal_aggregate",
        }
    }
}

impl fmt::Display for JdgSignatureScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error surfaced when parsing a JDG signature scheme from a string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid JDG signature scheme `{0}`")]
pub struct JdgSignatureSchemeParseError(pub String);

impl FromStr for JdgSignatureScheme {
    type Err = JdgSignatureSchemeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "simple_threshold" | "simple-threshold" | "simple" => Ok(Self::SimpleThreshold),
            "bls_normal_aggregate"
            | "bls-normal-aggregate"
            | "bls_normal"
            | "bls-normal"
            | "bls_aggregate"
            | "bls-aggregate"
            | "bls_preaggregated"
            | "bls-preaggregated" => Ok(Self::BlsNormalAggregate),
            other => Err(JdgSignatureSchemeParseError(other.to_string())),
        }
    }
}

/// Error surfaced when converting a signature scheme id into a known scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unsupported JDG signature scheme id {scheme_id}")]
pub struct JdgSignatureSchemeIdError {
    /// Raw scheme identifier.
    pub scheme_id: u16,
}

impl TryFrom<u16> for JdgSignatureScheme {
    type Error = JdgSignatureSchemeIdError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            JDG_SIGNATURE_SCHEME_SIMPLE_THRESHOLD => Ok(Self::SimpleThreshold),
            JDG_SIGNATURE_SCHEME_BLS_NORMAL_AGGREGATE => Ok(Self::BlsNormalAggregate),
            scheme_id => Err(JdgSignatureSchemeIdError { scheme_id }),
        }
    }
}

/// Threshold signature envelope; concrete scheme defined by JDG policy.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgThresholdSignature {
    /// Identifier for the signing scheme (e.g., simple threshold or BLS aggregate).
    pub scheme_id: u16,
    /// Optional LSB-first signer bitmap for threshold schemes.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub signer_bitmap: Option<Vec<u8>>,
    /// Raw signatures (ordered as defined by the scheme).
    pub signatures: Vec<Vec<u8>>,
}

/// Commitment to secret data stored under a Secret Data Node (SDN).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgSdnCommitment {
    /// Commitment format version.
    pub version: u16,
    /// Scope covered by the SDN commitment (jurisdiction, dataspace, block window).
    pub scope: JdgAttestationScope,
    /// Integrity hash of the encrypted payload stored under the SDN.
    pub encrypted_payload_hash: Hash,
    /// Signature or seal produced by the SDN over the signable commitment body.
    pub seal: SignatureOf<JdgSdnCommitmentSignable>,
    /// SDN public key used to generate the seal.
    pub sdn_public_key: PublicKey,
}

impl JdgSdnCommitment {
    /// Validate invariants that are independent of the attestation.
    ///
    /// # Errors
    ///
    /// Returns [`JdgSdnCommitmentError`] when the commitment is malformed.
    pub fn validate_basic(&self) -> Result<(), JdgSdnCommitmentError> {
        if self.version != JDG_SDN_COMMITMENT_VERSION_V1 {
            return Err(JdgSdnCommitmentError::UnsupportedVersion {
                version: self.version,
            });
        }
        self.scope
            .block_range
            .validate()
            .map_err(|_| JdgSdnCommitmentError::InvalidBlockRange)?;
        let seal: Signature = self.seal.clone().into();
        if seal.payload().is_empty() {
            return Err(JdgSdnCommitmentError::EmptySeal);
        }
        Ok(())
    }

    /// Compute the domain-tagged signing hash for the commitment body.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<JdgSdnCommitmentSignable> {
        let signable = JdgSdnCommitmentSignable::from(self);
        let mut encoded = Vec::new();
        signable.encode_to(&mut encoded);
        let mut payload =
            Vec::with_capacity(JDG_SDN_COMMITMENT_DOMAIN_TAG_V1.len() + encoded.len());
        payload.extend_from_slice(JDG_SDN_COMMITMENT_DOMAIN_TAG_V1);
        payload.extend_from_slice(&encoded);
        HashOf::from_untyped_unchecked(Hash::new(payload))
    }
}

/// Commitment body covered by the SDN seal.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgSdnCommitmentSignable {
    version: u16,
    scope: JdgAttestationScope,
    encrypted_payload_hash: Hash,
    sdn_public_key: PublicKey,
}

impl From<&JdgSdnCommitment> for JdgSdnCommitmentSignable {
    fn from(commitment: &JdgSdnCommitment) -> Self {
        Self {
            version: commitment.version,
            scope: commitment.scope.clone(),
            encrypted_payload_hash: commitment.encrypted_payload_hash,
            sdn_public_key: commitment.sdn_public_key.clone(),
        }
    }
}

/// Rotation/overlap policy applied when sealing SDN payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgSdnRotationPolicy {
    /// Number of blocks the previous SDN key remains valid after a successor activates.
    pub dual_publish_blocks: u64,
}

/// Policy describing how SDN commitments are enforced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgSdnPolicy {
    /// Whether SDN commitments are mandatory for the attested payload.
    pub require_commitments: bool,
    /// Rotation/overlap rules for SDN keys.
    pub rotation: JdgSdnRotationPolicy,
}

/// SDN sealing key with activation and retirement windows.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgSdnKeyRecord {
    /// SDN public key.
    pub public_key: PublicKey,
    /// First height (inclusive) where the key may be used.
    pub activated_at: u64,
    /// Last height (inclusive) where the key may be used. `None` means still active.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub retired_at: Option<u64>,
    /// Optional parent key this record replaces.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rotation_parent: Option<PublicKey>,
}

impl JdgSdnKeyRecord {
    /// Whether the key is active for the provided block range.
    #[must_use]
    pub fn is_active_for(&self, range: &JdgBlockRange) -> bool {
        range.start_height >= self.activated_at && range.end_height <= self.retirement_bound()
    }

    fn retirement_bound(&self) -> u64 {
        self.retired_at.unwrap_or(u64::MAX)
    }
}

/// Registry of SDN sealing keys keyed by algorithm+payload bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JdgSdnRegistry {
    keys: BTreeMap<(Algorithm, Vec<u8>), JdgSdnKeyRecord>,
}

impl JdgSdnRegistry {
    /// Register a new SDN key, wiring retirement for the parent according to the rotation policy.
    ///
    /// # Errors
    ///
    /// Returns [`JdgSdnRegistryError`] if the key already exists, the parent is missing,
    /// or the rotation window would violate the configured grace period.
    pub fn register_key(
        &mut self,
        record: JdgSdnKeyRecord,
        rotation_policy: &JdgSdnRotationPolicy,
    ) -> Result<(), JdgSdnRegistryError> {
        let fingerprint = Self::fingerprint(&record.public_key);
        if self.keys.contains_key(&fingerprint) {
            return Err(JdgSdnRegistryError::DuplicateKey);
        }

        if let Some(parent) = &record.rotation_parent {
            let parent_fingerprint = Self::fingerprint(parent);
            let parent_record = self.keys.get_mut(&parent_fingerprint).ok_or_else(|| {
                JdgSdnRegistryError::MissingParent {
                    parent: parent.clone(),
                }
            })?;
            if record.activated_at <= parent_record.activated_at {
                return Err(JdgSdnRegistryError::ActivationNotMonotonic {
                    activation: record.activated_at,
                    parent_activation: parent_record.activated_at,
                });
            }
            let allowed_retired_at = record
                .activated_at
                .saturating_add(rotation_policy.dual_publish_blocks);
            match parent_record.retired_at {
                Some(retired_at) if retired_at < record.activated_at => {
                    return Err(JdgSdnRegistryError::ParentAlreadyRetired {
                        parent_retired_at: retired_at,
                        activation: record.activated_at,
                    });
                }
                Some(retired_at) if retired_at > allowed_retired_at => {
                    return Err(JdgSdnRegistryError::OverlapExceedsPolicy {
                        activation: record.activated_at,
                        allowed_retired_at,
                        parent_retired_at: retired_at,
                    });
                }
                _ => parent_record.retired_at = Some(allowed_retired_at),
            }
        }

        self.keys.insert(fingerprint, record);
        Ok(())
    }

    /// Verify a single SDN commitment against the registry and policy.
    ///
    /// # Errors
    ///
    /// Returns [`JdgSdnValidationError`] when the commitment fails registry, key, or signature checks.
    pub fn verify_commitment(
        &self,
        commitment: &JdgSdnCommitment,
        index: usize,
        attestation_scope: &JdgAttestationScope,
        policy: &JdgSdnPolicy,
    ) -> Result<(), JdgSdnValidationError> {
        commitment
            .validate_basic()
            .map_err(|source| JdgSdnValidationError::Commitment { index, source })?;

        if commitment.scope != *attestation_scope {
            return Err(JdgSdnValidationError::ScopeMismatch { index });
        }

        let fingerprint = Self::fingerprint(&commitment.sdn_public_key);
        let record =
            self.keys
                .get(&fingerprint)
                .ok_or_else(|| JdgSdnValidationError::UnknownSdnKey {
                    index,
                    public_key: commitment.sdn_public_key.clone(),
                })?;

        if !record.is_active_for(&commitment.scope.block_range) {
            return Err(JdgSdnValidationError::InactiveSdnKey {
                index,
                activation: record.activated_at,
                retired_at: record.retired_at,
                start_height: commitment.scope.block_range.start_height,
                end_height: commitment.scope.block_range.end_height,
            });
        }

        // Honour rotation grace by ensuring the retirement window was clamped according to policy.
        if let Some(retired_at) = record.retired_at {
            let max_allowed = record
                .activated_at
                .saturating_add(policy.rotation.dual_publish_blocks);
            if retired_at > max_allowed {
                return Err(JdgSdnValidationError::InactiveSdnKey {
                    index,
                    activation: record.activated_at,
                    retired_at: record.retired_at,
                    start_height: commitment.scope.block_range.start_height,
                    end_height: commitment.scope.block_range.end_height,
                });
            }
        }

        commitment
            .seal
            .verify_hash(&commitment.sdn_public_key, commitment.signing_hash())
            .map_err(|_| JdgSdnValidationError::InvalidSeal { index })?;

        Ok(())
    }

    /// Fetch a key record by public key fingerprint.
    #[must_use]
    pub fn record(&self, key: &PublicKey) -> Option<&JdgSdnKeyRecord> {
        self.keys.get(&Self::fingerprint(key))
    }

    /// Iterate over all registered SDN key records.
    pub fn keys(&self) -> impl Iterator<Item = &JdgSdnKeyRecord> {
        self.keys.values()
    }

    fn fingerprint(key: &PublicKey) -> (Algorithm, Vec<u8>) {
        let (algorithm, bytes) = key.to_bytes();
        (algorithm, bytes.to_vec())
    }
}

/// Committee identifier bound into an attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgCommitteeId(pub [u8; 32]);

impl JdgCommitteeId {
    /// Construct a committee identifier from a 32-byte value.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// JDG attestation payload including scope, signer set, optional proof, and signature envelope.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgAttestation {
    /// Explicit attestation format version.
    pub version: u16,
    /// Dataspace/jurisdiction scope and covered block range.
    pub scope: JdgAttestationScope,
    /// Pre-state version/commit of the jurisdiction WSV prior to execution.
    pub pre_state_version: Hash,
    /// Canonical read/write access set touched by the execution.
    pub access_set: JdgStateAccessSet,
    /// Business-verdict outcome.
    pub verdict: JdgVerdict,
    /// Commitment to the post-state delta (hash or encrypted delta).
    pub post_state_delta: Vec<u8>,
    /// Jurisdiction state root after applying the delta.
    pub jurisdiction_root: Hash,
    /// Commitments/seals for secret data stored under SDNs.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub sdn_commitments: Vec<JdgSdnCommitment>,
    /// Optional data-availability acknowledgment for local storage.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub da_ack: Option<Vec<u8>>,
    /// Height after which the attestation expires (exclusive).
    pub expiry_height: u64,
    /// Committee identifier.
    pub committee_id: JdgCommitteeId,
    /// Required threshold for the committee.
    pub committee_threshold: u16,
    /// Hash of the normalized statement/policy evaluated by the JDG.
    pub statement_hash: Hash,
    /// Optional zero-knowledge proof backing the attestation.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub proof: Option<ProofBox>,
    /// Signer public keys (ordered, deduplicated).
    pub signer_set: Vec<PublicKey>,
    /// Attested epoch (permissioned mode uses `0`).
    pub epoch: u64,
    /// Attested block height (must fall inside `scope.block_range`).
    pub block_height: u64,
    /// Threshold signature over the domain-tagged attestation payload.
    pub signature: JdgThresholdSignature,
}

impl JdgAttestation {
    /// Compute a stable hash for signing using the v1 domain tag.
    #[must_use]
    pub fn signing_hash(&self) -> HashOf<JdgAttestationSignable> {
        let signable = JdgAttestationSignable::from(self);
        let mut encoded = Vec::new();
        signable.encode_to(&mut encoded);
        let mut payload = Vec::with_capacity(JDG_ATTESTATION_DOMAIN_TAG_V1.len() + encoded.len());
        payload.extend_from_slice(JDG_ATTESTATION_DOMAIN_TAG_V1);
        payload.extend_from_slice(&encoded);
        HashOf::from_untyped_unchecked(Hash::new(payload))
    }

    /// Hash the signer set to record the ordered signer commitment.
    #[must_use]
    pub fn signer_commitment(&self) -> HashOf<Vec<PublicKey>> {
        HashOf::new(&self.signer_set)
    }

    /// Validate attestation invariants (version, ranges, thresholds, signer set).
    ///
    /// # Errors
    ///
    /// Returns [`JdgAttestationError`] when any invariant fails.
    pub fn validate(&self) -> Result<(), JdgAttestationError> {
        self.validate_with_sdn(false)
    }

    /// Validate attestation invariants and, optionally, SDN commitments.
    ///
    /// Set `require_sdn_commitments` to `true` when the attestation must carry
    /// SDN commitments (e.g., for PII/secret payloads).
    ///
    /// # Errors
    ///
    /// Returns [`JdgAttestationError`] when any invariant fails.
    pub fn validate_with_sdn(
        &self,
        require_sdn_commitments: bool,
    ) -> Result<(), JdgAttestationError> {
        if self.version != JDG_ATTESTATION_VERSION_V1 {
            return Err(JdgAttestationError::UnsupportedVersion {
                version: self.version,
            });
        }
        self.scope.block_range.validate()?;
        if !self.scope.block_range.contains(self.block_height) {
            return Err(JdgAttestationError::BlockHeightOutOfRange {
                block_height: self.block_height,
                start_height: self.scope.block_range.start_height,
                end_height: self.scope.block_range.end_height,
            });
        }
        if self.committee_threshold == 0 {
            return Err(JdgAttestationError::ZeroCommitteeThreshold);
        }
        if self.signer_set.len() < usize::from(self.committee_threshold) {
            return Err(JdgAttestationError::SignerSetTooSmall {
                signers: self.signer_set.len(),
                threshold: self.committee_threshold,
            });
        }
        let unique: BTreeSet<_> = self.signer_set.iter().collect();
        if unique.len() != self.signer_set.len() {
            return Err(JdgAttestationError::DuplicateSigners);
        }
        self.validate_sdn_commitments(require_sdn_commitments)
    }

    fn validate_sdn_commitments(
        &self,
        require_sdn_commitments: bool,
    ) -> Result<(), JdgAttestationError> {
        if self.sdn_commitments.is_empty() {
            if require_sdn_commitments {
                return Err(JdgAttestationError::MissingSdnCommitments);
            }
            return Ok(());
        }

        let mut seen = BTreeSet::new();
        for (index, commitment) in self.sdn_commitments.iter().enumerate() {
            commitment
                .validate_basic()
                .map_err(|error| JdgAttestationError::SdnCommitment {
                    index,
                    source: error,
                })?;
            if commitment.scope != self.scope {
                return Err(JdgAttestationError::SdnScopeMismatch { index });
            }
            let (algorithm, key_bytes) = commitment.sdn_public_key.to_bytes();
            let dedup_key = (
                algorithm,
                key_bytes.to_vec(),
                commitment.encrypted_payload_hash,
            );
            if !seen.insert(dedup_key) {
                return Err(JdgAttestationError::DuplicateSdnCommitment { index });
            }
        }
        Ok(())
    }

    /// Validate SDN commitments structurally and against the provided registry/policy.
    ///
    /// # Errors
    ///
    /// Returns [`JdgSdnValidationError`] when structural validation fails, keys are missing or
    /// inactive, or seals do not verify.
    pub fn validate_with_sdn_registry(
        &self,
        registry: &JdgSdnRegistry,
        policy: &JdgSdnPolicy,
    ) -> Result<(), JdgSdnValidationError> {
        self.validate_with_sdn(policy.require_commitments)
            .map_err(|source| JdgSdnValidationError::Attestation { source })?;
        for (index, commitment) in self.sdn_commitments.iter().enumerate() {
            registry.verify_commitment(commitment, index, &self.scope, policy)?;
        }
        Ok(())
    }
}

/// Errors surfaced by [`JdgSdnCommitment`] validation.
#[derive(Debug, Copy, Clone, Error, PartialEq, Eq)]
pub enum JdgSdnCommitmentError {
    /// Commitment block range bounds are invalid.
    #[error("invalid SDN commitment block range")]
    InvalidBlockRange,
    /// SDN commitment version is unsupported.
    #[error("unsupported SDN commitment version {version}")]
    UnsupportedVersion {
        /// Version number.
        version: u16,
    },
    /// SDN seal must be non-empty.
    #[error("SDN seal must not be empty")]
    EmptySeal,
}

/// Errors surfaced by SDN key registration and rotation.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum JdgSdnRegistryError {
    /// Key already exists in the registry.
    #[error("SDN key already registered")]
    DuplicateKey,
    /// Referenced parent key is not present.
    #[error("parent SDN key is missing")]
    MissingParent {
        /// Public key of the missing parent.
        parent: PublicKey,
    },
    /// New activation height is not strictly after the parent activation.
    #[error(
        "activation height {activation} must be greater than parent activation {parent_activation}"
    )]
    ActivationNotMonotonic {
        /// Requested activation height.
        activation: u64,
        /// Parent activation height.
        parent_activation: u64,
    },
    /// Parent key already retired before the requested activation.
    #[error("parent retired at {parent_retired_at} before activation {activation}")]
    ParentAlreadyRetired {
        /// Parent retirement height.
        parent_retired_at: u64,
        /// Requested activation height.
        activation: u64,
    },
    /// Parent overlap would exceed the configured dual-publish policy.
    #[error(
        "parent retirement {parent_retired_at} exceeds dual-publish window ending at {allowed_retired_at}"
    )]
    OverlapExceedsPolicy {
        /// Requested activation height.
        activation: u64,
        /// Allowed retirement height per policy.
        allowed_retired_at: u64,
        /// Existing parent retirement height.
        parent_retired_at: u64,
    },
}

/// Errors surfaced by [`JdgAttestation`] validation.
#[derive(Debug, Copy, Clone, Error, PartialEq, Eq)]
pub enum JdgAttestationError {
    /// Block range bounds are invalid.
    #[error("invalid attestation block range {start_height}..={end_height}")]
    InvalidBlockRange {
        /// Start height.
        start_height: u64,
        /// End height.
        end_height: u64,
    },
    /// Attested block height falls outside the declared range.
    #[error(
        "block height {block_height} is outside the attested range {start_height}..={end_height}"
    )]
    BlockHeightOutOfRange {
        /// Height bound in the attestation.
        block_height: u64,
        /// Range start.
        start_height: u64,
        /// Range end.
        end_height: u64,
    },
    /// Committee threshold may not be zero.
    #[error("committee threshold must be non-zero")]
    ZeroCommitteeThreshold,
    /// Provided signer set is shorter than the committee threshold.
    #[error(
        "signer set shorter than committee threshold (signers={signers}, threshold={threshold})"
    )]
    SignerSetTooSmall {
        /// Total signers provided.
        signers: usize,
        /// Required threshold.
        threshold: u16,
    },
    /// Signer set contains duplicate keys.
    #[error("signer set contains duplicates")]
    DuplicateSigners,
    /// Unsupported attestation version.
    #[error("unsupported attestation version {version}")]
    UnsupportedVersion {
        /// Version number.
        version: u16,
    },
    /// SDN commitments are required but missing.
    #[error("SDN commitments required by policy but missing")]
    MissingSdnCommitments,
    /// SDN commitment failed basic validation.
    #[error("invalid SDN commitment at index {index}: {source}")]
    SdnCommitment {
        /// Position of the commitment in the attestation vector.
        index: usize,
        /// Underlying validation error.
        #[source]
        source: JdgSdnCommitmentError,
    },
    /// SDN commitment scope differs from the attestation scope.
    #[error("SDN commitment scope mismatch at index {index}")]
    SdnScopeMismatch {
        /// Position of the commitment in the attestation vector.
        index: usize,
    },
    /// SDN commitment duplicates an existing signer/payload pair.
    #[error("duplicate SDN commitment at index {index}")]
    DuplicateSdnCommitment {
        /// Position of the duplicate commitment.
        index: usize,
    },
}

/// Errors surfaced when validating SDN commitments against a registry.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum JdgSdnValidationError {
    /// Underlying attestation failed structural validation.
    #[error("attestation failed SDN validation: {source}")]
    Attestation {
        /// Structural validation error.
        #[source]
        source: JdgAttestationError,
    },
    /// Referenced SDN key was not found.
    #[error("SDN key at index {index} is not registered")]
    UnknownSdnKey {
        /// Commitment index.
        index: usize,
        /// Missing public key.
        public_key: PublicKey,
    },
    /// SDN key is inactive for the attested range.
    #[error(
        "SDN key at index {index} is inactive for range {start_height}..={end_height} (activation {activation}, retired_at {retired_at:?})"
    )]
    InactiveSdnKey {
        /// Commitment index.
        index: usize,
        /// Activation height.
        activation: u64,
        /// Retirement height.
        retired_at: Option<u64>,
        /// Range start height.
        start_height: u64,
        /// Range end height.
        end_height: u64,
    },
    /// Commitment scope differs from the attestation scope.
    #[error("SDN commitment scope mismatch at index {index}")]
    ScopeMismatch {
        /// Commitment index.
        index: usize,
    },
    /// SDN seal failed verification.
    #[error("invalid SDN seal at index {index}")]
    InvalidSeal {
        /// Commitment index.
        index: usize,
    },
    /// Structural SDN commitment validation failed.
    #[error("invalid SDN commitment at index {index}: {source}")]
    Commitment {
        /// Commitment index.
        index: usize,
        /// Underlying validation error.
        #[source]
        source: JdgSdnCommitmentError,
    },
}

/// View of [`JdgAttestation`] used for hashing/signing (omits the signature).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JdgAttestationSignable {
    version: u16,
    scope: JdgAttestationScope,
    pre_state_version: Hash,
    access_set: JdgStateAccessSet,
    verdict: JdgVerdict,
    post_state_delta: Vec<u8>,
    jurisdiction_root: Hash,
    sdn_commitments: Vec<JdgSdnCommitment>,
    da_ack: Option<Vec<u8>>,
    expiry_height: u64,
    committee_id: JdgCommitteeId,
    committee_threshold: u16,
    statement_hash: Hash,
    proof: Option<ProofBox>,
    signer_set: Vec<PublicKey>,
    epoch: u64,
    block_height: u64,
}

impl From<&JdgAttestation> for JdgAttestationSignable {
    fn from(value: &JdgAttestation) -> Self {
        Self {
            version: value.version,
            scope: value.scope.clone(),
            pre_state_version: value.pre_state_version,
            access_set: value.access_set.clone(),
            verdict: value.verdict,
            post_state_delta: value.post_state_delta.clone(),
            jurisdiction_root: value.jurisdiction_root,
            sdn_commitments: value.sdn_commitments.clone(),
            da_ack: value.da_ack.clone(),
            expiry_height: value.expiry_height,
            committee_id: value.committee_id,
            committee_threshold: value.committee_threshold,
            statement_hash: value.statement_hash,
            proof: value.proof.clone(),
            signer_set: value.signer_set.clone(),
            epoch: value.epoch,
            block_height: value.block_height,
        }
    }
}

fn normalize_keys(mut keys: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    keys.sort();
    keys.dedup();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sdn_commitment(
        scope: &JdgAttestationScope,
        keypair: &iroha_crypto::KeyPair,
    ) -> JdgSdnCommitment {
        let mut commitment = JdgSdnCommitment {
            version: JDG_SDN_COMMITMENT_VERSION_V1,
            scope: scope.clone(),
            encrypted_payload_hash: Hash::prehashed([0x77; 32]),
            seal: SignatureOf::from_signature(Signature::from_bytes(&[0u8])),
            sdn_public_key: keypair.public_key().clone(),
        };
        let seal = SignatureOf::from_hash(keypair.private_key(), commitment.signing_hash());
        commitment.seal = seal;
        commitment
    }

    fn sample_attestation() -> JdgAttestation {
        let jurisdiction_id = JurisdictionId::new(b"JUR1".to_vec()).expect("valid id");
        let scope = JdgAttestationScope {
            jurisdiction_id,
            dataspace: DataSpaceId::new(7),
            block_range: JdgBlockRange::new(10, 12).expect("valid range"),
        };
        let keypair = iroha_crypto::KeyPair::random();
        let signer = keypair.public_key().clone();
        let sdn_keypair = iroha_crypto::KeyPair::random();
        let sdn_commitment = sample_sdn_commitment(&scope, &sdn_keypair);
        JdgAttestation {
            version: JDG_ATTESTATION_VERSION_V1,
            scope,
            pre_state_version: Hash::prehashed([0x11; 32]),
            access_set: JdgStateAccessSet::normalized(vec![b"a".to_vec()], vec![b"b".to_vec()]),
            verdict: JdgVerdict::Accept,
            post_state_delta: vec![0xAA],
            jurisdiction_root: Hash::prehashed([0x22; 32]),
            sdn_commitments: vec![sdn_commitment],
            da_ack: None,
            expiry_height: 99,
            committee_id: JdgCommitteeId::new([0x33; 32]),
            committee_threshold: 1,
            statement_hash: Hash::prehashed([0x44; 32]),
            proof: None,
            signer_set: vec![signer],
            epoch: 0,
            block_height: 11,
            signature: JdgThresholdSignature {
                scheme_id: JDG_SIGNATURE_SCHEME_SIMPLE_THRESHOLD,
                signer_bitmap: Some(vec![0b0000_0001]),
                signatures: vec![vec![0x55]],
            },
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_decode_defaults_missing_sdn_commitments() {
        let mut attestation = sample_attestation();
        attestation.sdn_commitments.clear();

        let json = norito::json::to_json_pretty(&attestation).expect("serialize attestation");
        assert!(
            !json.contains("sdn_commitments"),
            "empty commitments should be elided: {json}"
        );

        let decoded: JdgAttestation =
            norito::json::from_str(&json).expect("deserialize attestation");
        assert!(decoded.sdn_commitments.is_empty());
        assert_eq!(decoded.scope, attestation.scope);
        assert_eq!(decoded.block_height, attestation.block_height);
        assert_eq!(decoded.expiry_height, attestation.expiry_height);
    }

    #[test]
    fn jurisdiction_id_bounds() {
        assert_eq!(
            JurisdictionId::new(Vec::new()).unwrap_err(),
            JurisdictionIdError::Empty
        );
        let long = vec![0u8; JDG_MAX_JURISDICTION_ID_LEN + 1];
        assert_eq!(
            JurisdictionId::new(long).unwrap_err(),
            JurisdictionIdError::TooLong {
                len: JDG_MAX_JURISDICTION_ID_LEN + 1,
                max: JDG_MAX_JURISDICTION_ID_LEN
            }
        );
    }

    #[test]
    fn block_range_validation() {
        let err = JdgBlockRange::new(5, 4).unwrap_err();
        assert_eq!(
            err,
            JdgAttestationError::InvalidBlockRange {
                start_height: 5,
                end_height: 4
            }
        );
    }

    #[test]
    fn attestation_signing_hash_uses_domain_tag() {
        let attestation = sample_attestation();
        let signable = JdgAttestationSignable::from(&attestation);
        let mut encoded = Vec::new();
        signable.encode_to(&mut encoded);
        let mut expected_payload =
            Vec::with_capacity(JDG_ATTESTATION_DOMAIN_TAG_V1.len() + encoded.len());
        expected_payload.extend_from_slice(JDG_ATTESTATION_DOMAIN_TAG_V1);
        expected_payload.extend_from_slice(&encoded);
        let expected = HashOf::from_untyped_unchecked(Hash::new(expected_payload.clone()));

        assert_eq!(attestation.signing_hash(), expected);
        assert_ne!(attestation.signing_hash(), HashOf::new(&signable));
    }

    #[test]
    fn signature_scheme_from_str_accepts_aliases() {
        assert_eq!(
            "simple_threshold".parse::<JdgSignatureScheme>().unwrap(),
            JdgSignatureScheme::SimpleThreshold
        );
        assert_eq!(
            "bls-aggregate".parse::<JdgSignatureScheme>().unwrap(),
            JdgSignatureScheme::BlsNormalAggregate
        );
    }

    #[test]
    fn signature_scheme_ids_and_labels() {
        assert_eq!(
            JdgSignatureScheme::SimpleThreshold.scheme_id(),
            JDG_SIGNATURE_SCHEME_SIMPLE_THRESHOLD
        );
        assert_eq!(
            JdgSignatureScheme::BlsNormalAggregate.scheme_id(),
            JDG_SIGNATURE_SCHEME_BLS_NORMAL_AGGREGATE
        );
        assert_eq!(
            JdgSignatureScheme::SimpleThreshold.as_str(),
            "simple_threshold"
        );
        assert_eq!(
            JdgSignatureScheme::BlsNormalAggregate.as_str(),
            "bls_normal_aggregate"
        );
        assert_eq!(
            JdgSignatureScheme::SimpleThreshold.to_string(),
            "simple_threshold"
        );
    }

    #[test]
    fn signature_scheme_try_from_id_rejects_unknown() {
        assert_eq!(
            JdgSignatureScheme::try_from(JDG_SIGNATURE_SCHEME_SIMPLE_THRESHOLD).unwrap(),
            JdgSignatureScheme::SimpleThreshold
        );
        assert_eq!(
            JdgSignatureScheme::try_from(JDG_SIGNATURE_SCHEME_BLS_NORMAL_AGGREGATE).unwrap(),
            JdgSignatureScheme::BlsNormalAggregate
        );
        let err = JdgSignatureScheme::try_from(99).unwrap_err();
        assert_eq!(err.scheme_id, 99);
    }

    #[test]
    fn attestation_validation_catches_signer_issues() {
        let mut attestation = sample_attestation();
        attestation.committee_threshold = 2;
        let err = attestation.validate().unwrap_err();
        assert_eq!(
            err,
            JdgAttestationError::SignerSetTooSmall {
                signers: 1,
                threshold: 2
            }
        );

        let duplicate_key = attestation.signer_set[0].clone();
        attestation.signer_set.push(duplicate_key);
        attestation.committee_threshold = 1;
        let err = attestation.validate().unwrap_err();
        assert_eq!(err, JdgAttestationError::DuplicateSigners);
    }

    #[test]
    fn attestation_validation_checks_block_range() {
        let mut attestation = sample_attestation();
        attestation.block_height = 20;
        let err = attestation.validate().unwrap_err();
        assert_eq!(
            err,
            JdgAttestationError::BlockHeightOutOfRange {
                block_height: 20,
                start_height: 10,
                end_height: 12
            }
        );
    }

    #[test]
    fn access_set_is_normalized() {
        let access_set = JdgStateAccessSet::normalized(
            vec![b"b".to_vec(), b"a".to_vec(), b"a".to_vec()],
            vec![b"z".to_vec(), b"y".to_vec(), b"z".to_vec()],
        );
        assert_eq!(access_set.reads, vec![b"a".to_vec(), b"b".to_vec()]);
        assert_eq!(access_set.writes, vec![b"y".to_vec(), b"z".to_vec()]);
    }

    #[test]
    fn attestation_requires_sdn_commitments_when_flagged() {
        let mut attestation = sample_attestation();
        attestation.sdn_commitments.clear();
        let err = attestation
            .validate_with_sdn(true)
            .expect_err("sdn commitments must be enforced when required");
        assert_eq!(err, JdgAttestationError::MissingSdnCommitments);
    }

    #[test]
    fn sdn_commitment_scope_mismatch_is_rejected() {
        let mut attestation = sample_attestation();
        attestation.sdn_commitments[0].scope.dataspace = DataSpaceId::new(99);
        let err = attestation
            .validate_with_sdn(true)
            .expect_err("scope mismatch should be rejected");
        match err {
            JdgAttestationError::SdnScopeMismatch { index, .. } => assert_eq!(index, 0),
            other => panic!("expected SdnScopeMismatch, got {other:?}"),
        }
    }

    #[test]
    fn sdn_commitment_empty_seal_is_rejected() {
        let mut attestation = sample_attestation();
        attestation.sdn_commitments[0].seal =
            SignatureOf::from_signature(Signature::from_bytes(&[]));
        let err = attestation
            .validate_with_sdn(true)
            .expect_err("empty SDN seals must be rejected");
        assert!(matches!(
            err,
            JdgAttestationError::SdnCommitment {
                source: JdgSdnCommitmentError::EmptySeal,
                ..
            }
        ));
    }

    #[test]
    fn sdn_commitment_invalid_range_is_rejected() {
        let mut attestation = sample_attestation();
        attestation.sdn_commitments[0].scope.block_range = JdgBlockRange {
            start_height: 15,
            end_height: 14,
        };
        let err = attestation
            .validate_with_sdn(true)
            .expect_err("invalid block range must be rejected");
        assert!(matches!(
            err,
            JdgAttestationError::SdnCommitment {
                source: JdgSdnCommitmentError::InvalidBlockRange,
                ..
            }
        ));
    }

    #[test]
    fn duplicate_sdn_commitments_are_rejected() {
        let mut attestation = sample_attestation();
        let duplicate = attestation.sdn_commitments[0].clone();
        attestation.sdn_commitments.push(duplicate);
        let err = attestation
            .validate_with_sdn(true)
            .expect_err("duplicate SDN commitments must be rejected");
        assert!(matches!(
            err,
            JdgAttestationError::DuplicateSdnCommitment { .. }
        ));
    }

    #[test]
    fn sdn_registry_accepts_active_commitment() {
        let attestation = sample_attestation();
        let mut registry = JdgSdnRegistry::default();
        let key = attestation.sdn_commitments[0].sdn_public_key.clone();
        let rotation = JdgSdnRotationPolicy {
            dual_publish_blocks: 2,
        };
        registry
            .register_key(
                JdgSdnKeyRecord {
                    public_key: key,
                    activated_at: 0,
                    retired_at: None,
                    rotation_parent: None,
                },
                &rotation,
            )
            .expect("register active key");

        let policy = JdgSdnPolicy {
            require_commitments: true,
            rotation,
        };

        attestation
            .validate_with_sdn_registry(&registry, &policy)
            .expect("registry validation should pass");
    }

    #[test]
    fn sdn_registry_rejects_unknown_key() {
        let attestation = sample_attestation();
        let policy = JdgSdnPolicy {
            require_commitments: true,
            rotation: JdgSdnRotationPolicy::default(),
        };
        let registry = JdgSdnRegistry::default();

        let err = attestation
            .validate_with_sdn_registry(&registry, &policy)
            .expect_err("missing registry entry should be rejected");
        match err {
            JdgSdnValidationError::UnknownSdnKey { index, .. } => assert_eq!(index, 0),
            other => panic!("expected UnknownSdnKey, got {other:?}"),
        }
    }

    #[test]
    fn sdn_registry_rejects_inactive_key() {
        let attestation = sample_attestation();
        let mut registry = JdgSdnRegistry::default();
        let policy = JdgSdnPolicy {
            require_commitments: true,
            rotation: JdgSdnRotationPolicy::default(),
        };

        let key = attestation.sdn_commitments[0].sdn_public_key.clone();
        registry
            .register_key(
                JdgSdnKeyRecord {
                    public_key: key,
                    activated_at: 20,
                    retired_at: None,
                    rotation_parent: None,
                },
                &policy.rotation,
            )
            .expect("register inactive key");

        let err = attestation
            .validate_with_sdn_registry(&registry, &policy)
            .expect_err("inactive key should be rejected");
        assert!(matches!(
            err,
            JdgSdnValidationError::InactiveSdnKey { index: 0, .. }
        ));
    }

    #[test]
    fn sdn_registry_rejects_bad_signature() {
        let mut attestation = sample_attestation();
        let policy = JdgSdnPolicy {
            require_commitments: true,
            rotation: JdgSdnRotationPolicy::default(),
        };
        let mut registry = JdgSdnRegistry::default();

        let key = attestation.sdn_commitments[0].sdn_public_key.clone();
        registry
            .register_key(
                JdgSdnKeyRecord {
                    public_key: key.clone(),
                    activated_at: 0,
                    retired_at: None,
                    rotation_parent: None,
                },
                &policy.rotation,
            )
            .expect("register key");

        attestation.sdn_commitments[0].seal =
            SignatureOf::from_signature(Signature::from_bytes(&[0xAA]));
        let err = attestation
            .validate_with_sdn_registry(&registry, &policy)
            .expect_err("invalid seal should be rejected");
        assert!(matches!(
            err,
            JdgSdnValidationError::InvalidSeal { index: 0 }
        ));
    }

    #[test]
    fn sdn_registry_sets_parent_retirement_window() {
        let rotation = JdgSdnRotationPolicy {
            dual_publish_blocks: 3,
        };
        let parent = iroha_crypto::KeyPair::random();
        let child = iroha_crypto::KeyPair::random();
        let mut registry = JdgSdnRegistry::default();

        registry
            .register_key(
                JdgSdnKeyRecord {
                    public_key: parent.public_key().clone(),
                    activated_at: 5,
                    retired_at: None,
                    rotation_parent: None,
                },
                &rotation,
            )
            .expect("register parent");
        registry
            .register_key(
                JdgSdnKeyRecord {
                    public_key: child.public_key().clone(),
                    activated_at: 20,
                    retired_at: None,
                    rotation_parent: Some(parent.public_key().clone()),
                },
                &rotation,
            )
            .expect("register child");

        let parent_record = registry.record(parent.public_key()).expect("parent record");
        assert_eq!(parent_record.retired_at, Some(23));
    }

    #[test]
    fn sdn_registry_rejects_overlap_beyond_policy() {
        let rotation = JdgSdnRotationPolicy {
            dual_publish_blocks: 1,
        };
        let parent = iroha_crypto::KeyPair::random();
        let child = iroha_crypto::KeyPair::random();
        let mut registry = JdgSdnRegistry::default();

        registry
            .register_key(
                JdgSdnKeyRecord {
                    public_key: parent.public_key().clone(),
                    activated_at: 1,
                    retired_at: Some(50),
                    rotation_parent: None,
                },
                &rotation,
            )
            .expect("register parent");

        let err = registry
            .register_key(
                JdgSdnKeyRecord {
                    public_key: child.public_key().clone(),
                    activated_at: 10,
                    retired_at: None,
                    rotation_parent: Some(parent.public_key().clone()),
                },
                &rotation,
            )
            .expect_err("overlap beyond policy should be rejected");
        assert!(matches!(
            err,
            JdgSdnRegistryError::OverlapExceedsPolicy { .. }
        ));
    }
}
