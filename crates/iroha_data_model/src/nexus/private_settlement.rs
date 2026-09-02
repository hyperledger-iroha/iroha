//! Atomic private cross-dataspace settlement wire types.
//!
//! This module defines the public manifest, restricted proof sidecar, local
//! auditor policy and approval, and compact Native AMX receipt carried by the
//! first atomic private-settlement protocol. Business contents exist only in
//! [`PrivateSettlementAuditPlaintextV1`] and its nested restricted types. The
//! literal pool-to-asset mapping and its random opening additionally exist in
//! [`PrivateSettlementPoolGovernanceV1`]. These restricted objects have
//! redacted debug output and must never be embedded in blocks, public receipts,
//! logs, events, metrics, or errors.

use super::{DataSpaceId, LaneId};
use crate::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    peer::PeerId,
    privacy::{
        PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1, PrivacyCommitmentV1,
        PrivacyEncryptedOutputV1, PrivacyNullifierV1, PrivacyPoolIdV1, PrivacyRootV1,
    },
    transaction::FeePaymentIntent,
};
use iroha_crypto::{Hash, HashOf, HybridPublicKey, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use thiserror::Error;

/// Atomic private-settlement wire version.
pub const ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1: u8 = 1;
/// Minimum number of dataspaces in one atomic private settlement.
pub const ATOMIC_PRIVATE_SETTLEMENT_MIN_LEGS_V1: usize = 2;
/// Maximum number of dataspaces in one atomic private settlement.
pub const ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1: usize = u8::MAX as usize;
/// Exact number of nullifier slots in the fixed-shape settlement proof.
pub const PRIVATE_SETTLEMENT_INPUT_SLOTS_V1: usize = 2;
/// Exact number of output slots in the fixed-shape settlement proof.
pub const PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1: usize = 3;
/// Exact validator count in one private-settlement participant committee.
pub const PRIVATE_SETTLEMENT_COMMITTEE_VALIDATORS_V1: usize = 4;
/// Minimum signatures required from a four-validator participant committee.
pub const PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1: u8 = 3;
/// Maximum number of auditors governed by one dataspace policy.
pub const PRIVATE_SETTLEMENT_MAX_AUDITORS_V1: usize = 32;
/// Maximum cleartext memo bytes admitted to an auditor capsule.
pub const PRIVATE_SETTLEMENT_MAX_AUDIT_MEMO_BYTES_V1: usize = 2 * 1024;
/// Maximum governed policy-reference digests admitted to an auditor capsule.
pub const PRIVATE_SETTLEMENT_MAX_AUDIT_POLICY_REFERENCES_V1: usize = 32;
/// Maximum proof bytes in one restricted private-settlement sidecar.
///
/// This is the closed IVM proof ceiling.  Sidecar framing, capsule bytes, and
/// availability metadata are budgeted separately and must never be smuggled
/// into this field.
pub const PRIVATE_SETTLEMENT_MAX_PROOF_BYTES_V1: usize = 8 * 1024 * 1024;
/// ML-KEM-768 public key width.
pub const PRIVATE_SETTLEMENT_ML_KEM_768_PUBLIC_KEY_BYTES_V1: usize = 1_184;
/// ML-KEM-768 encapsulation ciphertext width.
pub const PRIVATE_SETTLEMENT_ML_KEM_768_CIPHERTEXT_BYTES_V1: usize = 1_088;
/// XChaCha20-Poly1305 nonce width.
pub const PRIVATE_SETTLEMENT_XCHACHA_NONCE_BYTES_V1: usize = 24;
/// A wrapped 32-byte DEK plus its Poly1305 authentication tag.
pub const PRIVATE_SETTLEMENT_WRAPPED_DEK_BYTES_V1: usize = 48;
/// Conservative canonical framing budget for the capsule header, AAD, nonce, and vectors.
pub const PRIVATE_SETTLEMENT_CAPSULE_ENVELOPE_BOUND_BYTES_V1: u64 = 8 * 1024;
/// Conservative canonical framing budget for one governed auditor's complete wrapped-DEK row.
///
/// The budget includes the account identifier, algorithm-tagged public key,
/// ML-KEM ciphertext, X25519 component, nonce, wrapped key, and Norito framing.
pub const PRIVATE_SETTLEMENT_WRAPPED_DEK_ROW_BOUND_BYTES_V1: u64 = 8 * 1024;
/// Upper bound for the canonical public carrier receipt.
pub const PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1: usize = 4 * 1024 * 1024;
/// Compressed BLS-normal proof/signature width used by Native AMX.
pub const PRIVATE_SETTLEMENT_BLS_BYTES_V1: usize = 96;
/// Attested lifecycle code for a sidecar collecting auditor approvals.
pub const PRIVATE_SETTLEMENT_LIFECYCLE_COLLECTING_V1: u8 = 0;
/// Attested lifecycle code for a sidecar with its auditor threshold satisfied.
pub const PRIVATE_SETTLEMENT_LIFECYCLE_AUDITED_V1: u8 = 1;
/// Attested lifecycle code for a durably prepared sidecar.
pub const PRIVATE_SETTLEMENT_LIFECYCLE_PREPARED_V1: u8 = 2;
/// Attested lifecycle code for a sidecar carrying a durable Commit QC.
pub const PRIVATE_SETTLEMENT_LIFECYCLE_COMMIT_CERTIFIED_V1: u8 = 3;
/// Attested lifecycle code for an atomically finalized sidecar.
pub const PRIVATE_SETTLEMENT_LIFECYCLE_FINALIZED_V1: u8 = 4;
/// Attested lifecycle code for an authoritatively aborted sidecar.
pub const PRIVATE_SETTLEMENT_LIFECYCLE_ABORTED_V1: u8 = 5;
/// Attested lifecycle code for an expired sidecar.
pub const PRIVATE_SETTLEMENT_LIFECYCLE_EXPIRED_V1: u8 = 6;
/// Exact number of ordinary fee-bearing carriers in a successful V1 settlement.
///
/// Prepare registration is the first carrier and atomic financial finalization
/// is the second. The designated private reimbursement terms bind this count so
/// they cannot be interpreted as covering only the final carrier.
pub const PRIVATE_SETTLEMENT_SUCCESS_FEE_BEARING_CARRIERS_V1: u8 = 2;
/// Exact audited settlement-local proof profile descriptor.
pub const PRIVATE_SETTLEMENT_PROOF_PROFILE_DESCRIPTOR_V1: &[u8] = b"iroha-atomic-private-settlement-stark-v1:native-rust:first-release:inputs=2-fixed:payer-authorization=purpose-separated-controller-signatures:outputs=3-fixed:roles=recipient+change+sponsor-reimbursement:selectors=canonical-active-or-domain-dummy:values=u128-checked-balanced:asset=salted-hidden-binding:tree=sha256-depth32:successor=proof-statement-bound-root+epoch:successor-correctness=validator-derived-frontier:public-intent=canonical-proof-binding-excluding-post-proof-artifacts:reimbursement-success-fee-carriers=2:business-plaintext=auditor-capsule-sha256-commitment:wallet=x25519+xchacha20poly1305:proof=stark-fri-sha256-goldilocks";

/// Return a deterministic safe upper bound for one canonical V1 audit capsule.
///
/// `padded_plaintext_bytes` excludes the 16-byte payload authentication tag.
/// The bound is intentionally conservative so configuration validation can
/// prove that at least the governed minimum auditor roster is usable without
/// constructing identities or cryptographic material.
#[must_use]
pub const fn private_settlement_capsule_canonical_upper_bound_v1(
    padded_plaintext_bytes: u64,
    auditor_count: u64,
) -> u64 {
    padded_plaintext_bytes
        .saturating_add(16)
        .saturating_add(PRIVATE_SETTLEMENT_CAPSULE_ENVELOPE_BOUND_BYTES_V1)
        .saturating_add(
            auditor_count.saturating_mul(PRIVATE_SETTLEMENT_WRAPPED_DEK_ROW_BOUND_BYTES_V1),
        )
}

const BUNDLE_ID_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:bundle-id:v1\0";
const PROOF_BINDING_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:proof-binding:v1\0";
const MANIFEST_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:manifest:v1\0";
const STATEMENT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:statement:v1\0";
const PROOF_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:proof:v1\0";
const DELTA_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:delta:v1\0";
const CAPSULE_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:audit-capsule:v1\0";
const SIDECAR_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:sidecar:v1\0";
const SIDECAR_AVAILABILITY_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:availability-signature:v1\0";
const AUDITOR_VIEW_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:auditor-view:v1\0";
const AUDITOR_VIEW_ATTESTATION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:auditor-view-attestation:v1\0";
const AUDIT_APPROVAL_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:audit-approval:v1\0";
const AUDIT_APPROVAL_ACKNOWLEDGEMENT_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:audit-approval-acknowledgement:v1\0";
const AUDIT_APPROVAL_ACKNOWLEDGEMENT_ATTESTATION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:audit-approval-acknowledgement-attestation:v1\0";
const PHASE_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:participant-phase-signature:v1\0";
const PREPARED_BUNDLE_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:prepared-bundle:v1\0";
const AUDIT_POLICY_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:audit-policy:v1\0";
const POOL_GOVERNANCE_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:pool-governance:v1\0";
const AUDITOR_KEY_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:auditor-key:v1\0";
const AUTHORITY_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:authority:v1\0";
const OUTPUT_VIEW_KEY_AUTHORIZATION_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:output-view-key-authorization:v1\0";
const PAYER_INPUT_AUTHORIZATION_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:payer-input-authorization:v1\0";
const AUDIT_PLAINTEXT_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:audit-plaintext:v1\0";
const ASSET_BINDING_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:asset-binding:v1\0";
const FEE_INTENT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:nexus:private-settlement:fee-intent:v1\0";
const REIMBURSEMENT_TERMS_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:nexus:private-settlement:reimbursement-terms:v1\0";

fn canonical_hash<T: Encode>(domain: &[u8], value: &T) -> Result<Hash, norito::Error> {
    let encoded = norito::encode_canonical(value)?;
    let encoded_len = u64::try_from(encoded.len())
        .map_err(|_| norito::Error::Io(std::io::Error::other("canonical payload is too large")))?;
    Ok(Hash::new_from_chunks(&[
        domain,
        &encoded_len.to_le_bytes(),
        encoded.as_slice(),
    ]))
}

fn hash_is_zero(hash: &Hash) -> bool {
    *hash == Hash::prehashed([0; Hash::LENGTH])
}

/// Compute the SHA-256 commitment to one canonical typed auditor plaintext.
///
/// The decoder rejects arbitrary bytes and non-canonical aliases before the
/// non-circular audit projection is committed.  Output commitments and output
/// memo digests are verifier-derived from that projection and are deliberately
/// not hashed back into it; an auditor must recompute and compare both fields.
///
/// # Errors
///
/// Returns a Norito error on a platform where the slice length does not fit
/// the canonical `u64` frame.
pub fn private_settlement_audit_plaintext_commitment_v1(
    canonical_plaintext: &[u8],
) -> Result<Hash, norito::Error> {
    let plaintext =
        norito::decode_canonical::<PrivateSettlementAuditPlaintextV1>(canonical_plaintext)?;
    plaintext.validate().map_err(|_| {
        norito::Error::Io(std::io::Error::other(
            "invalid atomic private settlement audit plaintext",
        ))
    })?;
    plaintext.commitment()
}

/// Canonical route and incarnation of one private settlement leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementRouteV1 {
    /// Participant dataspace visible on the global plane.
    pub dataspace_id: DataSpaceId,
    /// Participant lane visible on the global plane.
    pub lane_id: LaneId,
    /// Exact active lane incarnation at the authority context.
    pub lane_incarnation: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct PrivateSettlementAssetBindingMaterialV1 {
    route: PrivateSettlementRouteV1,
    pool_id: PrivacyPoolIdV1,
    asset_definition_id: AssetDefinitionId,
    asset_binding_salt: [u8; 32],
}

/// Recompute the salted commitment opening one restricted pool-to-asset mapping.
///
/// The complete route, including the lane incarnation, is committed so a valid
/// opening cannot be replayed after a route is re-incarnated. The exact asset
/// and random salt are restricted governance material and must not be copied
/// into a public manifest, receipt, event, log, or metric.
///
/// # Errors
///
/// Returns a Norito error when the canonical binding material cannot be encoded.
pub fn private_settlement_asset_binding_commitment_v1(
    route: PrivateSettlementRouteV1,
    pool_id: PrivacyPoolIdV1,
    asset_definition_id: &AssetDefinitionId,
    asset_binding_salt: [u8; 32],
) -> Result<Hash, norito::Error> {
    let material = PrivateSettlementAssetBindingMaterialV1 {
        route,
        pool_id,
        asset_definition_id: asset_definition_id.clone(),
        asset_binding_salt,
    };
    let encoded = norito::encode_canonical(&material)?;
    let encoded_len = u64::try_from(encoded.len())
        .map_err(|_| norito::Error::Io(std::io::Error::other("asset binding is too large")))?;
    let mut hasher = Sha256::new();
    hasher.update(ASSET_BINDING_COMMITMENT_DOMAIN_V1);
    hasher.update(encoded_len.to_le_bytes());
    hasher.update(encoded);
    Ok(Hash::prehashed(hasher.finalize().into()))
}

/// Public commitment to one restricted private settlement leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementLegCommitmentV1 {
    /// Zero-based ordinal in canonical route order.
    pub ordinal: u8,
    /// Exact participant route.
    pub route: PrivateSettlementRouteV1,
    /// Opaque private-note pool identifier; it never embeds a literal asset identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Salted commitment to the restricted pool-to-asset binding.
    pub asset_binding_commitment: Hash,
    /// Digest of the governed local auditor policy.
    pub audit_policy_digest: Hash,
    /// Digest of the complete restricted sidecar.
    pub payload_digest: Hash,
    /// Digest of the exact signed restricted-DA availability certificate.
    pub availability_certificate_digest: Hash,
    /// Digest of the fixed-shape public state delta.
    pub delta_digest: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct PrivateSettlementBundleIdMaterialV1 {
    network_id: NetworkId,
    authority_context_height: u64,
    expiry_height: u64,
    sponsor: AccountId,
    fee_intent_digest: Hash,
    reimbursement_terms_commitment: Hash,
    reimbursement_leg_ordinal: u8,
    legs: Vec<PrivateSettlementBundleLegMaterialV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
struct PrivateSettlementBundleLegMaterialV1 {
    ordinal: u8,
    route: PrivateSettlementRouteV1,
    pool_id: PrivacyPoolIdV1,
    asset_binding_commitment: Hash,
    audit_policy_digest: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct PrivateSettlementProofBindingMaterialV1 {
    version: u8,
    bundle_id: Hash,
    intent: PrivateSettlementBundleIdMaterialV1,
}

/// Public manifest for one atomic private cross-dataspace settlement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AtomicPrivateSettlementV1 {
    /// Wire version; must be [`ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1`].
    pub version: u8,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Stable domain-separated identifier derived from public intent material.
    pub bundle_id: Hash,
    /// Global/catalog height used to resolve routes, policies, and keys.
    pub authority_context_height: u64,
    /// Final global block height at which unfinished work expires.
    pub expiry_height: u64,
    /// Neutral public relayer that submits and funds the global carrier.
    pub sponsor: AccountId,
    /// Exact signature-bound public fee payer, assets, and charge maxima.
    pub public_fee_intent: FeePaymentIntent,
    /// Digest of the signature-bound public fee intent and quote.
    pub fee_intent_digest: Hash,
    /// Commitment to the privately agreed sponsor reimbursement terms.
    pub reimbursement_terms_commitment: Hash,
    /// Leg whose fixed third output reimburses the sponsor.
    pub reimbursement_leg_ordinal: u8,
    /// Canonically ordered public leg commitments.
    pub legs: Vec<PrivateSettlementLegCommitmentV1>,
}

impl AtomicPrivateSettlementV1 {
    /// Supported manifest version.
    pub const VERSION: u8 = ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1;

    fn bundle_id_material(&self) -> PrivateSettlementBundleIdMaterialV1 {
        PrivateSettlementBundleIdMaterialV1 {
            network_id: self.network_id,
            authority_context_height: self.authority_context_height,
            expiry_height: self.expiry_height,
            sponsor: self.sponsor.clone(),
            fee_intent_digest: self.fee_intent_digest,
            reimbursement_terms_commitment: self.reimbursement_terms_commitment,
            reimbursement_leg_ordinal: self.reimbursement_leg_ordinal,
            legs: self
                .legs
                .iter()
                .map(|leg| PrivateSettlementBundleLegMaterialV1 {
                    ordinal: leg.ordinal,
                    route: leg.route,
                    pool_id: leg.pool_id,
                    asset_binding_commitment: leg.asset_binding_commitment,
                    audit_policy_digest: leg.audit_policy_digest,
                })
                .collect(),
        }
    }

    /// Compute the stable bundle identifier from public intent material.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical material cannot be encoded.
    pub fn computed_bundle_id(&self) -> Result<Hash, norito::Error> {
        canonical_hash(BUNDLE_ID_DOMAIN_V1, &self.bundle_id_material())
    }

    /// Compute the proof transcript's canonical public-intent digest.
    ///
    /// This projection binds every settlement intent field and every ordered
    /// participant route, pool, asset binding, and audit policy. It
    /// deliberately excludes the payload, restricted-availability, and delta
    /// digests because those artifacts are produced after the proof. The final
    /// manifest, committee certificates, carrier, and receipt continue to bind
    /// those exact post-proof digests through [`Self::manifest_digest`].
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical material cannot be encoded.
    pub fn proof_binding_digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(
            PROOF_BINDING_DIGEST_DOMAIN_V1,
            &PrivateSettlementProofBindingMaterialV1 {
                version: self.version,
                bundle_id: self.bundle_id,
                intent: self.bundle_id_material(),
            },
        )
    }

    /// Compute the digest of the exact public fee intent.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the fee intent cannot be encoded.
    pub fn computed_fee_intent_digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(FEE_INTENT_DIGEST_DOMAIN_V1, &self.public_fee_intent)
    }

    /// Compute the digest of the exact manifest, including sidecar and delta commitments.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the manifest cannot be encoded.
    pub fn manifest_digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(MANIFEST_DIGEST_DOMAIN_V1, self)
    }

    /// Validate participant bounds, canonical ordering, expiry, and all commitments.
    ///
    /// # Errors
    ///
    /// Returns a typed fail-closed structural error.
    pub fn validate(&self) -> Result<(), PrivateSettlementValidationError> {
        self.validate_with_availability_v1(true)
    }

    /// Validate a pre-certification manifest whose availability digests are reserved zeroes.
    ///
    /// All settlement intent, payload, and delta commitments are final at this
    /// boundary. Only the per-leg availability-certificate digests remain
    /// unset so every committee signs the same immutable sidecar material.
    ///
    /// # Errors
    ///
    /// Returns a typed fail-closed structural error.
    pub fn validate_provisional(&self) -> Result<(), PrivateSettlementValidationError> {
        self.validate_with_availability_v1(false)
    }

    fn validate_with_availability_v1(
        &self,
        certificates_are_final: bool,
    ) -> Result<(), PrivateSettlementValidationError> {
        if self.version != Self::VERSION {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if self.authority_context_height == 0 || self.expiry_height <= self.authority_context_height
        {
            return Err(PrivateSettlementValidationError::InvalidExpiry);
        }
        if !(ATOMIC_PRIVATE_SETTLEMENT_MIN_LEGS_V1..=ATOMIC_PRIVATE_SETTLEMENT_MAX_LEGS_V1)
            .contains(&self.legs.len())
        {
            return Err(PrivateSettlementValidationError::ParticipantCount {
                count: self.legs.len(),
            });
        }
        if usize::from(self.reimbursement_leg_ordinal) >= self.legs.len() {
            return Err(PrivateSettlementValidationError::InvalidReimbursementLeg);
        }
        if self.public_fee_intent.validate().is_err() {
            return Err(PrivateSettlementValidationError::InvalidFeeIntent);
        }
        if hash_is_zero(&self.fee_intent_digest)
            || hash_is_zero(&self.reimbursement_terms_commitment)
        {
            return Err(PrivateSettlementValidationError::ZeroCommitment);
        }
        let computed_fee_intent_digest = self
            .computed_fee_intent_digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if self.fee_intent_digest != computed_fee_intent_digest {
            return Err(PrivateSettlementValidationError::FeeIntentDigestMismatch);
        }
        let mut previous_route = None;
        let mut participant_dataspaces = BTreeSet::new();
        for (index, leg) in self.legs.iter().enumerate() {
            let expected =
                u8::try_from(index).expect("private settlement has at most 255 participant legs");
            if leg.ordinal != expected {
                return Err(PrivateSettlementValidationError::NonCanonicalOrdinal {
                    index,
                    actual: leg.ordinal,
                });
            }
            if previous_route.is_some_and(|previous| previous >= leg.route) {
                return Err(PrivateSettlementValidationError::NonCanonicalRouteOrder);
            }
            previous_route = Some(leg.route);
            if !participant_dataspaces.insert(leg.route.dataspace_id) {
                return Err(PrivateSettlementValidationError::DuplicateDataspace);
            }
            if hash_is_zero(&leg.route.lane_incarnation)
                || leg.pool_id.is_zero()
                || hash_is_zero(&leg.asset_binding_commitment)
                || hash_is_zero(&leg.audit_policy_digest)
                || hash_is_zero(&leg.payload_digest)
                || hash_is_zero(&leg.delta_digest)
                || (certificates_are_final && hash_is_zero(&leg.availability_certificate_digest))
                || (!certificates_are_final && !hash_is_zero(&leg.availability_certificate_digest))
            {
                return Err(PrivateSettlementValidationError::ZeroCommitment);
            }
        }
        let computed = self
            .computed_bundle_id()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if self.bundle_id != computed {
            return Err(PrivateSettlementValidationError::BundleIdMismatch);
        }
        Ok(())
    }
}

/// Proof relation selected for a restricted private settlement leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "profile",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum PrivateSettlementProofProfileV1 {
    /// IVM private-note STARK with two fixed inputs and three fixed outputs.
    IvmPrivateNoteFixed2In3Out,
}

impl PrivateSettlementProofProfileV1 {
    /// Compute the pinned digest of this proof relation and wire profile.
    #[must_use]
    pub fn digest(self) -> Hash {
        match self {
            Self::IvmPrivateNoteFixed2In3Out => Hash::prehashed(
                Sha256::digest(PRIVATE_SETTLEMENT_PROOF_PROFILE_DESCRIPTOR_V1).into(),
            ),
        }
    }
}

/// Exact fixed-shape public statement verified by a participant committee.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementProofStatementV1 {
    /// Statement wire version.
    pub version: u8,
    /// Closed proof profile.
    pub profile: PrivateSettlementProofProfileV1,
    /// Pinned digest of the exact relation/profile descriptor.
    pub proof_profile_digest: Hash,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Stable settlement bundle identifier.
    pub bundle_id: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Exact participant route.
    pub route: PrivateSettlementRouteV1,
    /// Global/catalog height used to resolve the route, keys, and policy.
    pub authority_context_height: u64,
    /// Opaque private-note pool identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Salted commitment to the restricted pool-to-asset binding.
    pub asset_binding_commitment: Hash,
    /// Current private state root.
    pub old_root: PrivacyRootV1,
    /// Successor root bound into the proof statement and independently derived by validators.
    pub new_root: PrivacyRootV1,
    /// Epoch of `old_root`.
    pub old_epoch: u64,
    /// Epoch of `new_root`; exactly one greater than `old_epoch`.
    pub new_epoch: u64,
    /// Two fixed nullifier slots, including any domain-separated dummy slot.
    pub nullifiers: Vec<PrivacyNullifierV1>,
    /// Three fixed commitment slots: recipient, change/dummy, and sponsor reimbursement.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// Three fixed encrypted outputs aligned with `output_commitments`.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
    /// SHA-256 commitment to the exact auditor-only business plaintext.
    pub audit_plaintext_commitment: Hash,
    /// Digest of the encrypted audit capsule.
    pub audit_capsule_digest: Hash,
    /// Digest of the governed auditor policy.
    pub audit_policy_digest: Hash,
    /// Exact auditor encryption/signing key epoch.
    pub audit_key_epoch: u64,
    /// Digest of the public fee quote and fee intent.
    pub fee_intent_digest: Hash,
    /// Commitment to private reimbursement terms.
    pub reimbursement_terms_commitment: Hash,
    /// Manifest leg whose third output must reimburse the sponsor.
    pub reimbursement_leg_ordinal: u8,
    /// Final global height at which the statement remains admissible.
    pub expiry_height: u64,
}

impl PrivateSettlementProofStatementV1 {
    /// Compute the domain-separated statement digest.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the statement cannot be encoded.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(STATEMENT_DIGEST_DOMAIN_V1, self)
    }

    /// Validate the complete fixed-shape public relation boundary.
    ///
    /// # Errors
    ///
    /// Returns a typed structural error before any expensive proof verification.
    pub fn validate(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if self.proof_profile_digest != self.profile.digest()
            || hash_is_zero(&self.bundle_id)
            || hash_is_zero(&self.route.lane_incarnation)
            || self.pool_id.is_zero()
            || hash_is_zero(&self.asset_binding_commitment)
            || self.old_root.is_zero()
            || self.new_root.is_zero()
            || hash_is_zero(&self.audit_plaintext_commitment)
            || hash_is_zero(&self.audit_capsule_digest)
            || hash_is_zero(&self.audit_policy_digest)
            || hash_is_zero(&self.fee_intent_digest)
            || hash_is_zero(&self.reimbursement_terms_commitment)
        {
            return Err(PrivateSettlementValidationError::ZeroCommitment);
        }
        if self.old_root == self.new_root
            || self.authority_context_height == 0
            || self.old_epoch == 0
            || self.old_epoch.checked_add(1) != Some(self.new_epoch)
            || self.audit_key_epoch == 0
            || self.expiry_height <= self.authority_context_height
        {
            return Err(PrivateSettlementValidationError::InvalidEpoch);
        }
        if self.nullifiers.len() != PRIVATE_SETTLEMENT_INPUT_SLOTS_V1
            || self.output_commitments.len() != PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1
            || self.encrypted_outputs.len() != PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1
        {
            return Err(PrivateSettlementValidationError::InvalidFixedSlotCount {
                nullifiers: self.nullifiers.len(),
                outputs: self.output_commitments.len(),
            });
        }
        if self.nullifiers.iter().any(PrivacyNullifierV1::is_zero)
            || self
                .output_commitments
                .iter()
                .any(PrivacyCommitmentV1::is_zero)
        {
            return Err(PrivateSettlementValidationError::ZeroCommitment);
        }
        if self.nullifiers[0] == self.nullifiers[1]
            || self
                .output_commitments
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1
        {
            return Err(PrivateSettlementValidationError::DuplicateStateItem);
        }
        if self
            .encrypted_outputs
            .iter()
            .map(|output| output.recipient)
            .collect::<BTreeSet<_>>()
            .len()
            != PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1
        {
            return Err(PrivateSettlementValidationError::DuplicateStateItem);
        }
        for (index, output) in self.encrypted_outputs.iter().enumerate() {
            if output.recipient.is_zero()
                || output.ephemeral_public_key.is_zero()
                || output.commitment != self.output_commitments[index]
                || output.ciphertext.len() != PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1
                || output.ciphertext.get(..4) != Some(b"IPNE".as_slice())
                || output.ciphertext[4..4 + PRIVATE_SETTLEMENT_XCHACHA_NONCE_BYTES_V1]
                    .iter()
                    .all(|byte| *byte == 0)
                || output.ciphertext[4 + PRIVATE_SETTLEMENT_XCHACHA_NONCE_BYTES_V1..]
                    .iter()
                    .all(|byte| *byte == 0)
            {
                return Err(PrivateSettlementValidationError::InvalidEncryptedOutput { index });
            }
        }
        Ok(())
    }
}

/// Fixed-shape state delta committed by a participant committee.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementDeltaV1 {
    /// Delta wire version.
    pub version: u8,
    /// Stable settlement bundle identifier.
    pub bundle_id: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Exact participant route.
    pub route: PrivateSettlementRouteV1,
    /// Opaque pool identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Salted commitment to the restricted asset binding.
    pub asset_binding_commitment: Hash,
    /// Current root and epoch.
    pub old_root: PrivacyRootV1,
    /// Successor root deterministically derived from the old frontier and fixed outputs.
    pub new_root: PrivacyRootV1,
    /// Epoch of `old_root`.
    pub old_epoch: u64,
    /// Epoch of `new_root`; exactly one greater than `old_epoch`.
    pub new_epoch: u64,
    /// Fixed nullifier slots.
    pub nullifiers: Vec<PrivacyNullifierV1>,
    /// Fixed output commitments.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// Fixed encrypted outputs aligned with `output_commitments`.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
    /// Digest of the proof statement.
    pub statement_digest: Hash,
    /// Digest of the proof bytes.
    pub proof_digest: Hash,
    /// Digest of the encrypted audit capsule.
    pub capsule_digest: Hash,
    /// Digest of the governed audit policy.
    pub audit_policy_digest: Hash,
    /// Auditor policy key epoch.
    pub audit_key_epoch: u64,
}

impl PrivateSettlementDeltaV1 {
    /// Compute the domain-separated canonical delta digest.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the delta cannot be encoded.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(DELTA_DIGEST_DOMAIN_V1, self)
    }

    /// Validate the complete public delta without requiring restricted proof material.
    ///
    /// This is the validation boundary used by global receipt admission. It
    /// deliberately repeats the fixed-shape checks from the proof statement so
    /// a committee certificate can never make malformed public state data
    /// admissible by itself.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a reserved digest, malformed route, invalid
    /// epoch transition, duplicate state item, or malformed encrypted output.
    pub fn validate_public_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if hash_is_zero(&self.bundle_id)
            || hash_is_zero(&self.route.lane_incarnation)
            || self.pool_id.is_zero()
            || hash_is_zero(&self.asset_binding_commitment)
            || self.old_root.is_zero()
            || self.new_root.is_zero()
            || hash_is_zero(&self.statement_digest)
            || hash_is_zero(&self.proof_digest)
            || hash_is_zero(&self.capsule_digest)
            || hash_is_zero(&self.audit_policy_digest)
        {
            return Err(PrivateSettlementValidationError::ZeroCommitment);
        }
        if self.old_root == self.new_root
            || self.old_epoch == 0
            || self.audit_key_epoch == 0
            || self.old_epoch.checked_add(1) != Some(self.new_epoch)
        {
            return Err(PrivateSettlementValidationError::InvalidEpoch);
        }
        if self.nullifiers.len() != PRIVATE_SETTLEMENT_INPUT_SLOTS_V1
            || self.output_commitments.len() != PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1
            || self.encrypted_outputs.len() != PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1
        {
            return Err(PrivateSettlementValidationError::InvalidFixedSlotCount {
                nullifiers: self.nullifiers.len(),
                outputs: self.output_commitments.len(),
            });
        }
        if self.nullifiers.iter().any(PrivacyNullifierV1::is_zero)
            || self
                .output_commitments
                .iter()
                .any(PrivacyCommitmentV1::is_zero)
        {
            return Err(PrivateSettlementValidationError::ZeroCommitment);
        }
        if self
            .nullifiers
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != PRIVATE_SETTLEMENT_INPUT_SLOTS_V1
            || self
                .output_commitments
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1
        {
            return Err(PrivateSettlementValidationError::DuplicateStateItem);
        }
        if self
            .encrypted_outputs
            .iter()
            .map(|output| output.recipient)
            .collect::<BTreeSet<_>>()
            .len()
            != PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1
        {
            return Err(PrivateSettlementValidationError::DuplicateStateItem);
        }
        for (index, output) in self.encrypted_outputs.iter().enumerate() {
            if output.recipient.is_zero()
                || output.ephemeral_public_key.is_zero()
                || output.commitment != self.output_commitments[index]
                || output.ciphertext.len() != PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1
                || output.ciphertext.get(..4) != Some(b"IPNE".as_slice())
                || output.ciphertext[4..4 + PRIVATE_SETTLEMENT_XCHACHA_NONCE_BYTES_V1]
                    .iter()
                    .all(|byte| *byte == 0)
                || output.ciphertext[4 + PRIVATE_SETTLEMENT_XCHACHA_NONCE_BYTES_V1..]
                    .iter()
                    .all(|byte| *byte == 0)
            {
                return Err(PrivateSettlementValidationError::InvalidEncryptedOutput { index });
            }
        }
        Ok(())
    }

    /// Validate fixed slot shape and alignment with its proof statement.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed or substituted state material.
    pub fn validate_against(
        &self,
        statement: &PrivateSettlementProofStatementV1,
    ) -> Result<(), PrivateSettlementValidationError> {
        statement.validate()?;
        self.validate_public_shape()?;
        let expected_statement_digest = statement
            .digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if self.statement_digest != expected_statement_digest
            || self.bundle_id != statement.bundle_id
            || self.leg_ordinal != statement.leg_ordinal
            || self.route != statement.route
            || self.pool_id != statement.pool_id
            || self.asset_binding_commitment != statement.asset_binding_commitment
            || self.old_root != statement.old_root
            || self.new_root != statement.new_root
            || self.old_epoch != statement.old_epoch
            || self.new_epoch != statement.new_epoch
            || self.nullifiers != statement.nullifiers
            || self.output_commitments != statement.output_commitments
            || self.encrypted_outputs != statement.encrypted_outputs
            || self.capsule_digest != statement.audit_capsule_digest
            || self.audit_policy_digest != statement.audit_policy_digest
            || self.audit_key_epoch != statement.audit_key_epoch
        {
            return Err(PrivateSettlementValidationError::DeltaStatementMismatch);
        }
        Ok(())
    }
}

/// Padding class used for encrypted auditor capsules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "padding",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum PrivateSettlementCapsulePaddingV1 {
    /// 4 KiB padded plaintext.
    KiB4,
    /// 16 KiB padded plaintext.
    KiB16,
    /// 64 KiB padded plaintext.
    KiB64,
    /// 256 KiB padded plaintext.
    KiB256,
}

impl PrivateSettlementCapsulePaddingV1 {
    /// Exact padded plaintext width.
    #[must_use]
    pub const fn plaintext_bytes(self) -> usize {
        match self {
            Self::KiB4 => 4 * 1024,
            Self::KiB16 => 16 * 1024,
            Self::KiB64 => 64 * 1024,
            Self::KiB256 => 256 * 1024,
        }
    }

    /// Exact ciphertext width including the Poly1305 tag.
    #[must_use]
    pub const fn ciphertext_bytes(self) -> usize {
        self.plaintext_bytes() + 16
    }
}

/// Fixed semantic role of one private-settlement output slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "role",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum PrivateSettlementAuditOutputRoleV1 {
    /// CBDC note created for the settlement recipient.
    SettlementRecipient,
    /// Optional change note returned to the payer.
    PayerChange,
    /// Optional note reimbursing the public carrier sponsor.
    SponsorReimbursement,
}

/// One fixed input slot authorized by the private-settlement payer.
///
/// The slot carries only public note identifiers and a digest of the note
/// spending authority. It never contains the corresponding spending secret.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditPayerInputV1 {
    /// Fixed input ordinal in the two-input relation.
    pub input_ordinal: u8,
    /// Whether this slot contains a spendable input rather than a cover note.
    pub active: bool,
    /// Exact input note commitment consumed by the proof witness.
    pub commitment: PrivacyCommitmentV1,
    /// Exact fixed-slot nullifier published by the proof statement.
    pub nullifier: PrivacyNullifierV1,
    /// Digest of the note spending authority, never its spending secret.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub note_spending_authority: [u8; 32],
    /// Non-zero bundle-bound dummy domain exactly when `active` is false.
    #[norito(required)]
    pub dummy_domain: Option<Hash>,
}

impl fmt::Debug for PrivateSettlementAuditPayerInputV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditPayerInputV1(<redacted>)")
    }
}

/// Purpose-separated authorization body for both fixed payer input slots.
///
/// The payer controller signs this restricted body before proof construction,
/// binding the exact public nullifiers and private input-authority digests to
/// one bundle, leg, route, and expiry without disclosing spending secrets.
#[derive(Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditPayerAuthorizationBodyV1 {
    /// Wire version.
    pub version: u8,
    /// Hash of the fixed payer-authorization purpose domain.
    pub purpose: Hash,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Stable settlement bundle identifier.
    pub bundle_id: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Exact participant route.
    pub route: PrivateSettlementRouteV1,
    /// Exact payer whose controller authorizes both input slots.
    pub payer: AccountId,
    /// Global settlement expiry preventing authorization replay after expiry.
    pub expiry_height: u64,
    /// Exactly two ordinal-ordered active-or-dummy input bindings.
    pub inputs: Vec<PrivateSettlementAuditPayerInputV1>,
}

impl fmt::Debug for PrivateSettlementAuditPayerAuthorizationBodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditPayerAuthorizationBodyV1(<redacted>)")
    }
}

impl PrivateSettlementAuditPayerAuthorizationBodyV1 {
    fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1
            || self.purpose != Hash::new(PAYER_INPUT_AUTHORIZATION_DOMAIN_V1)
            || hash_is_zero(&self.bundle_id)
            || hash_is_zero(&self.route.lane_incarnation)
            || self.expiry_height == 0
            || self.inputs.len() != PRIVATE_SETTLEMENT_INPUT_SLOTS_V1
        {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        for (index, input) in self.inputs.iter().enumerate() {
            if usize::from(input.input_ordinal) != index
                || input.commitment.is_zero()
                || input.nullifier.is_zero()
                || input.note_spending_authority.iter().all(|byte| *byte == 0)
                || input.active == input.dummy_domain.is_some()
                || input
                    .dummy_domain
                    .is_some_and(|dummy_domain| hash_is_zero(&dummy_domain))
            {
                return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
            }
        }
        if self.inputs[0].nullifier == self.inputs[1].nullifier {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        Ok(())
    }
}

/// One controller-member signature authorizing both fixed payer inputs.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditPayerSignatureV1 {
    /// Controller member that produced the signature.
    pub signer: PublicKey,
    /// Purpose-specific signature over the exact payer authorization body.
    pub signature: SignatureOf<PrivateSettlementAuditPayerAuthorizationBodyV1>,
}

impl fmt::Debug for PrivateSettlementAuditPayerSignatureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditPayerSignatureV1(<redacted>)")
    }
}

impl PrivateSettlementAuditPayerSignatureV1 {
    /// Construct one typed controller signature entry.
    #[must_use]
    pub fn new(
        signer: PublicKey,
        signature: SignatureOf<PrivateSettlementAuditPayerAuthorizationBodyV1>,
    ) -> Self {
        Self { signer, signature }
    }
}

/// Canonical single- or multisignature payer authorization for both inputs.
#[derive(Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditPayerAuthorizationV1 {
    /// Exact purpose-separated payer authorization body.
    pub body: PrivateSettlementAuditPayerAuthorizationBodyV1,
    /// Unique controller signatures in strict public-key order.
    pub signatures: Vec<PrivateSettlementAuditPayerSignatureV1>,
}

impl fmt::Debug for PrivateSettlementAuditPayerAuthorizationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditPayerAuthorizationV1(<redacted>)")
    }
}

impl PrivateSettlementAuditPayerAuthorizationV1 {
    /// Construct an authorization with canonical signer ordering.
    #[must_use]
    pub fn new(
        body: PrivateSettlementAuditPayerAuthorizationBodyV1,
        mut signatures: Vec<PrivateSettlementAuditPayerSignatureV1>,
    ) -> Self {
        signatures.sort_unstable_by(|left, right| left.signer.cmp(&right.signer));
        Self { body, signatures }
    }

    fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        self.body.validate_shape()?;
        if self.signatures.is_empty()
            || self
                .signatures
                .windows(2)
                .any(|pair| pair[0].signer >= pair[1].signer)
        {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        Ok(())
    }
}

/// Purpose-separated account authorization of one one-time output view key.
///
/// This body is restricted audit-capsule material. It binds a one-time X25519
/// view key to the exact account occupying one fixed settlement role without
/// publishing either the account controller or its authorization signatures.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditViewKeyAuthorizationBodyV1 {
    /// Wire version.
    pub version: u8,
    /// Hash of the fixed authorization-purpose domain.
    pub purpose: Hash,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Stable settlement bundle identifier.
    pub bundle_id: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Exact participant route.
    pub route: PrivateSettlementRouteV1,
    /// Fixed output ordinal within the three-output relation.
    pub output_ordinal: u8,
    /// Exact semantic output role.
    pub role: PrivateSettlementAuditOutputRoleV1,
    /// Exact account whose controller authorizes the view key.
    pub authorized_account: AccountId,
    /// One-time X25519 public view key being authorized.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_view_key: [u8; 32],
    /// Whether the authorized output is an active note or a fixed cover slot.
    pub output_active: bool,
    /// Digest of the authorized output note's spending authority.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub note_spending_authority: [u8; 32],
    /// Global settlement expiry preventing authorization replay after expiry.
    pub expiry_height: u64,
}

impl fmt::Debug for PrivateSettlementAuditViewKeyAuthorizationBodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditViewKeyAuthorizationBodyV1(<redacted>)")
    }
}

/// One controller-member signature authorizing an output view key.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditViewKeySignatureV1 {
    /// Controller member that produced the signature.
    pub signer: PublicKey,
    /// Purpose-specific signature over the exact authorization body.
    pub signature: SignatureOf<PrivateSettlementAuditViewKeyAuthorizationBodyV1>,
}

impl fmt::Debug for PrivateSettlementAuditViewKeySignatureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditViewKeySignatureV1(<redacted>)")
    }
}

impl PrivateSettlementAuditViewKeySignatureV1 {
    /// Construct one typed controller signature entry.
    #[must_use]
    pub fn new(
        signer: PublicKey,
        signature: SignatureOf<PrivateSettlementAuditViewKeyAuthorizationBodyV1>,
    ) -> Self {
        Self { signer, signature }
    }
}

/// Canonical single- or multisignature authorization for one output view key.
#[derive(Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditViewKeyAuthorizationV1 {
    /// Exact purpose-separated authorization body.
    pub body: PrivateSettlementAuditViewKeyAuthorizationBodyV1,
    /// Unique controller signatures in strict public-key order.
    pub signatures: Vec<PrivateSettlementAuditViewKeySignatureV1>,
}

impl fmt::Debug for PrivateSettlementAuditViewKeyAuthorizationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditViewKeyAuthorizationV1(<redacted>)")
    }
}

impl PrivateSettlementAuditViewKeyAuthorizationV1 {
    /// Construct an authorization with canonical signer ordering.
    #[must_use]
    pub fn new(
        body: PrivateSettlementAuditViewKeyAuthorizationBodyV1,
        mut signatures: Vec<PrivateSettlementAuditViewKeySignatureV1>,
    ) -> Self {
        signatures.sort_unstable_by(|left, right| left.signer.cmp(&right.signer));
        Self { body, signatures }
    }

    fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.body.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1
            || self.body.purpose != Hash::new(OUTPUT_VIEW_KEY_AUTHORIZATION_DOMAIN_V1)
            || self.body.expiry_height == 0
            || self.body.recipient_view_key.iter().all(|byte| *byte == 0)
            || self.signatures.is_empty()
            || self
                .signatures
                .windows(2)
                .any(|pair| pair[0].signer >= pair[1].signer)
        {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        Ok(())
    }
}

/// Capsule-only opening of the ephemeral X25519 output-encryption public key.
///
/// This is encryption randomness, not a note spending secret. An auditor uses
/// it with the public one-time view key to authenticate and open the published
/// ciphertext deterministically; it must never leave the restricted capsule.
#[derive(Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditEncryptionOpeningV1 {
    /// Ephemeral X25519 secret whose public key appears in the output envelope.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ephemeral_secret: [u8; 32],
}

impl fmt::Debug for PrivateSettlementAuditEncryptionOpeningV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditEncryptionOpeningV1(<redacted>)")
    }
}

/// Auditor-visible opening of one fixed private-note slot.
///
/// The opening deliberately excludes every spending secret.  Active slots
/// carry the values needed to recompute their commitments; inactive slots
/// carry only a domain-separated dummy identifier.
#[derive(Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditNoteOpeningV1 {
    /// Whether this slot represents a spendable note.
    pub active: bool,
    /// Public commitment aligned with the private proof witness.
    pub commitment: PrivacyCommitmentV1,
    /// Atomic CBDC value; zero exactly for a dummy slot.
    pub value: u128,
    /// Digest of the spending authority, never the spending secret itself.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub spending_authority: [u8; 32],
    /// Unique active-note nonce.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub rho: [u8; 32],
    /// Active-note commitment blinding.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub blinding: [u8; 32],
    /// Digest of the note-local memo.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub memo_digest: [u8; 32],
    /// Non-zero, bundle-bound dummy domain exactly when `active` is false.
    #[norito(required)]
    pub dummy_domain: Option<Hash>,
}

impl fmt::Debug for PrivateSettlementAuditNoteOpeningV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditNoteOpeningV1(<redacted>)")
    }
}

impl PrivateSettlementAuditNoteOpeningV1 {
    fn validate(self) -> Result<(), PrivateSettlementValidationError> {
        if self.commitment.is_zero() {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        let has_zero_private_field = self.spending_authority.iter().all(|byte| *byte == 0)
            || self.rho.iter().all(|byte| *byte == 0)
            || self.blinding.iter().all(|byte| *byte == 0)
            || self.memo_digest.iter().all(|byte| *byte == 0);
        if self.active {
            if self.value == 0 || has_zero_private_field || self.dummy_domain.is_some() {
                return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
            }
        } else if self.value != 0
            || has_zero_private_field
            || self.dummy_domain.is_none_or(|domain| hash_is_zero(&domain))
        {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        Ok(())
    }
}

/// Auditor-visible opening and one-time view key for one fixed output slot.
#[derive(Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditOutputV1 {
    /// Fixed position-dependent role.
    pub role: PrivateSettlementAuditOutputRoleV1,
    /// One-time recipient/view key, including a unique cover key for dummies.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub recipient_view_key: [u8; 32],
    /// Exact role-account authorization of `recipient_view_key`.
    pub view_key_authorization: PrivateSettlementAuditViewKeyAuthorizationV1,
    /// Capsule-only opening used to authenticate the published ciphertext.
    pub encryption_opening: PrivateSettlementAuditEncryptionOpeningV1,
    /// Note or domain-separated dummy opening.
    pub note: PrivateSettlementAuditNoteOpeningV1,
}

impl fmt::Debug for PrivateSettlementAuditOutputV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditOutputV1(<redacted>)")
    }
}

/// Exact business and note-opening material encrypted for local auditors.
///
/// This type is never public protocol state.  Its commitment is public and is
/// checked by the settlement proof; its bytes are available only after an
/// authorized auditor unwraps the capsule DEK.
#[derive(Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditPlaintextV1 {
    /// Plaintext wire version.
    pub version: u8,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Stable settlement bundle identifier.
    pub bundle_id: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Exact participant route.
    pub route: PrivateSettlementRouteV1,
    /// Opaque pool identifier aligned with the public statement.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact payer visible only to authorized local auditors.
    pub payer: AccountId,
    /// Purpose-separated payer-controller authorization of both input slots.
    pub payer_authorization: PrivateSettlementAuditPayerAuthorizationV1,
    /// Exact settlement recipient visible only to authorized local auditors.
    pub recipient: AccountId,
    /// Exact public carrier sponsor.
    pub sponsor: AccountId,
    /// Exact governed CBDC asset definition.
    pub asset_definition_id: AssetDefinitionId,
    /// Random salt opening the public asset-binding commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub asset_binding_salt: [u8; 32],
    /// Atomic value delivered to the settlement recipient.
    pub amount: u128,
    /// Atomic sponsor reimbursement; zero on every non-designated leg.
    pub sponsor_reimbursement_amount: u128,
    /// Exact public fee-intent digest bound into reimbursement terms.
    pub fee_intent_digest: Hash,
    /// Global settlement expiry bound into reimbursement terms.
    pub settlement_expiry_height: u64,
    /// Random opening salt for the private reimbursement-terms commitment.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub reimbursement_terms_salt: [u8; 32],
    /// Exact business memo, bounded before encryption.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub memo: Vec<u8>,
    /// Strictly ordered exact governance/policy references.
    pub policy_references: Vec<Hash>,
    /// Exactly two active-or-dummy input note openings.
    pub inputs: Vec<PrivateSettlementAuditNoteOpeningV1>,
    /// Exactly three role-ordered active-or-dummy outputs.
    pub outputs: Vec<PrivateSettlementAuditOutputV1>,
}

#[derive(Clone, Encode)]
struct PrivateSettlementAuditOutputCommitmentMaterialV1 {
    role: PrivateSettlementAuditOutputRoleV1,
    recipient_view_key: [u8; 32],
    view_key_authorization: PrivateSettlementAuditViewKeyAuthorizationV1,
    encryption_opening: PrivateSettlementAuditEncryptionOpeningV1,
    active: bool,
    value: u128,
    spending_authority: [u8; 32],
    rho: [u8; 32],
    blinding: [u8; 32],
    dummy_domain: Option<Hash>,
}

#[derive(Clone, Encode)]
struct PrivateSettlementAuditCommitmentMaterialV1 {
    version: u8,
    network_id: NetworkId,
    bundle_id: Hash,
    leg_ordinal: u8,
    route: PrivateSettlementRouteV1,
    pool_id: PrivacyPoolIdV1,
    payer: AccountId,
    payer_authorization: PrivateSettlementAuditPayerAuthorizationV1,
    recipient: AccountId,
    sponsor: AccountId,
    asset_definition_id: AssetDefinitionId,
    asset_binding_salt: [u8; 32],
    amount: u128,
    sponsor_reimbursement_amount: u128,
    fee_intent_digest: Hash,
    settlement_expiry_height: u64,
    reimbursement_terms_salt: [u8; 32],
    memo: Vec<u8>,
    policy_references: Vec<Hash>,
    inputs: Vec<PrivateSettlementAuditNoteOpeningV1>,
    outputs: Vec<PrivateSettlementAuditOutputCommitmentMaterialV1>,
}

#[derive(Clone, Encode)]
struct PrivateSettlementReimbursementTermsMaterialV1 {
    network_id: NetworkId,
    leg_ordinal: u8,
    route: PrivateSettlementRouteV1,
    sponsor: AccountId,
    asset_definition_id: AssetDefinitionId,
    sponsor_reimbursement_amount: u128,
    fee_intent_digest: Hash,
    success_fee_bearing_carriers: u8,
    settlement_expiry_height: u64,
    reimbursement_terms_salt: [u8; 32],
}

impl fmt::Debug for PrivateSettlementAuditPlaintextV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditPlaintextV1(<redacted>)")
    }
}

impl PrivateSettlementAuditPlaintextV1 {
    /// Derive the exact purpose-separated authorization body for both inputs.
    ///
    /// Public nullifiers are supplied by the proof statement; every remaining
    /// field is derived from this restricted plaintext. The fixed slot shape is
    /// enforced before a body is returned.
    ///
    /// # Errors
    ///
    /// Returns a redacted shape error unless exactly two non-zero, distinct
    /// statement nullifiers are supplied.
    pub fn payer_authorization_body(
        &self,
        nullifiers: &[PrivacyNullifierV1],
    ) -> Result<PrivateSettlementAuditPayerAuthorizationBodyV1, PrivateSettlementValidationError>
    {
        if nullifiers.len() != PRIVATE_SETTLEMENT_INPUT_SLOTS_V1
            || nullifiers.iter().any(PrivacyNullifierV1::is_zero)
            || nullifiers[0] == nullifiers[1]
            || self.inputs.len() != PRIVATE_SETTLEMENT_INPUT_SLOTS_V1
        {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        let inputs = self
            .inputs
            .iter()
            .zip(nullifiers)
            .enumerate()
            .map(|(index, (opening, nullifier))| {
                Ok(PrivateSettlementAuditPayerInputV1 {
                    input_ordinal: u8::try_from(index)
                        .map_err(|_| PrivateSettlementValidationError::InvalidAuditPlaintext)?,
                    active: opening.active,
                    commitment: opening.commitment,
                    nullifier: *nullifier,
                    note_spending_authority: opening.spending_authority,
                    dummy_domain: opening.dummy_domain,
                })
            })
            .collect::<Result<Vec<_>, PrivateSettlementValidationError>>()?;
        let body = PrivateSettlementAuditPayerAuthorizationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            purpose: Hash::new(PAYER_INPUT_AUTHORIZATION_DOMAIN_V1),
            network_id: self.network_id,
            bundle_id: self.bundle_id,
            leg_ordinal: self.leg_ordinal,
            route: self.route,
            payer: self.payer.clone(),
            expiry_height: self.settlement_expiry_height,
            inputs,
        };
        body.validate_shape()?;
        Ok(body)
    }

    /// Derive the exact purpose-separated authorization body for one output.
    ///
    /// The account is selected by fixed ordinal rather than by the untrusted
    /// role field: recipient, payer change, then sponsor.
    ///
    /// # Errors
    ///
    /// Returns a redacted shape error when `output_index` is outside the fixed
    /// three-output relation.
    pub fn output_view_key_authorization_body(
        &self,
        output_index: usize,
    ) -> Result<PrivateSettlementAuditViewKeyAuthorizationBodyV1, PrivateSettlementValidationError>
    {
        let output = self
            .outputs
            .get(output_index)
            .ok_or(PrivateSettlementValidationError::InvalidAuditPlaintext)?;
        let (role, authorized_account) = match output_index {
            0 => (
                PrivateSettlementAuditOutputRoleV1::SettlementRecipient,
                self.recipient.clone(),
            ),
            1 => (
                PrivateSettlementAuditOutputRoleV1::PayerChange,
                self.payer.clone(),
            ),
            2 => (
                PrivateSettlementAuditOutputRoleV1::SponsorReimbursement,
                self.sponsor.clone(),
            ),
            _ => return Err(PrivateSettlementValidationError::InvalidAuditPlaintext),
        };
        Ok(PrivateSettlementAuditViewKeyAuthorizationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            purpose: Hash::new(OUTPUT_VIEW_KEY_AUTHORIZATION_DOMAIN_V1),
            network_id: self.network_id,
            bundle_id: self.bundle_id,
            leg_ordinal: self.leg_ordinal,
            route: self.route,
            output_ordinal: u8::try_from(output_index)
                .map_err(|_| PrivateSettlementValidationError::InvalidAuditPlaintext)?,
            role,
            authorized_account,
            recipient_view_key: output.recipient_view_key,
            output_active: output.note.active,
            note_spending_authority: output.note.spending_authority,
            expiry_height: self.settlement_expiry_height,
        })
    }

    fn commitment_material(&self) -> PrivateSettlementAuditCommitmentMaterialV1 {
        PrivateSettlementAuditCommitmentMaterialV1 {
            version: self.version,
            network_id: self.network_id,
            bundle_id: self.bundle_id,
            leg_ordinal: self.leg_ordinal,
            route: self.route,
            pool_id: self.pool_id,
            payer: self.payer.clone(),
            payer_authorization: self.payer_authorization.clone(),
            recipient: self.recipient.clone(),
            sponsor: self.sponsor.clone(),
            asset_definition_id: self.asset_definition_id.clone(),
            asset_binding_salt: self.asset_binding_salt,
            amount: self.amount,
            sponsor_reimbursement_amount: self.sponsor_reimbursement_amount,
            fee_intent_digest: self.fee_intent_digest,
            settlement_expiry_height: self.settlement_expiry_height,
            reimbursement_terms_salt: self.reimbursement_terms_salt,
            memo: self.memo.clone(),
            policy_references: self.policy_references.clone(),
            inputs: self.inputs.clone(),
            outputs: self
                .outputs
                .iter()
                .map(|output| PrivateSettlementAuditOutputCommitmentMaterialV1 {
                    role: output.role,
                    recipient_view_key: output.recipient_view_key,
                    view_key_authorization: output.view_key_authorization.clone(),
                    encryption_opening: output.encryption_opening,
                    active: output.note.active,
                    value: output.note.value,
                    spending_authority: output.note.spending_authority,
                    rho: output.note.rho,
                    blinding: output.note.blinding,
                    dummy_domain: output.note.dummy_domain,
                })
                .collect(),
        }
    }

    /// Compute the non-circular SHA-256 commitment proved by the settlement relation.
    ///
    /// Every business field, input opening, output secret opening, role, role
    /// authorization, one-time view key, and ephemeral encryption opening is
    /// committed. Output memo digests and commitments are excluded because the
    /// verifier derives them from this commitment; the auditor separately
    /// recomputes and compares those public fields.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when canonical encoding fails.
    pub fn commitment(&self) -> Result<Hash, norito::Error> {
        let encoded = norito::encode_canonical(&self.commitment_material())?;
        let encoded_len = u64::try_from(encoded.len()).map_err(|_| {
            norito::Error::Io(std::io::Error::other(
                "audit commitment material is too large",
            ))
        })?;
        let mut hasher = Sha256::new();
        hasher.update(AUDIT_PLAINTEXT_COMMITMENT_DOMAIN_V1);
        hasher.update(encoded_len.to_le_bytes());
        hasher.update(encoded);
        Ok(Hash::prehashed(hasher.finalize().into()))
    }

    /// Compute the salted public binding of the exact restricted asset.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when canonical asset material cannot be encoded.
    pub fn asset_binding_commitment(&self) -> Result<Hash, norito::Error> {
        private_settlement_asset_binding_commitment_v1(
            self.route,
            self.pool_id,
            &self.asset_definition_id,
            self.asset_binding_salt,
        )
    }

    /// Compute the private sponsor-reimbursement terms commitment.
    ///
    /// The commitment deliberately excludes `bundle_id` because the bundle ID
    /// already commits to this value. Including both would create a circular
    /// fixed-point construction.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when canonical reimbursement material cannot be encoded.
    pub fn reimbursement_terms_commitment(&self) -> Result<Hash, norito::Error> {
        canonical_hash(
            REIMBURSEMENT_TERMS_COMMITMENT_DOMAIN_V1,
            &self.reimbursement_terms_material(PRIVATE_SETTLEMENT_SUCCESS_FEE_BEARING_CARRIERS_V1),
        )
    }

    fn reimbursement_terms_material(
        &self,
        success_fee_bearing_carriers: u8,
    ) -> PrivateSettlementReimbursementTermsMaterialV1 {
        PrivateSettlementReimbursementTermsMaterialV1 {
            network_id: self.network_id,
            leg_ordinal: self.leg_ordinal,
            route: self.route,
            sponsor: self.sponsor.clone(),
            asset_definition_id: self.asset_definition_id.clone(),
            sponsor_reimbursement_amount: self.sponsor_reimbursement_amount,
            fee_intent_digest: self.fee_intent_digest,
            success_fee_bearing_carriers,
            settlement_expiry_height: self.settlement_expiry_height,
            reimbursement_terms_salt: self.reimbursement_terms_salt,
        }
    }

    /// Validate the restricted plaintext's fixed slot shape and value balance.
    ///
    /// # Errors
    ///
    /// Returns a typed error without exposing any sensitive field value.
    pub fn validate(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if hash_is_zero(&self.bundle_id)
            || hash_is_zero(&self.route.lane_incarnation)
            || self.pool_id.is_zero()
            || self.asset_binding_salt.iter().all(|byte| *byte == 0)
            || hash_is_zero(&self.fee_intent_digest)
            || self.settlement_expiry_height == 0
            || self.reimbursement_terms_salt.iter().all(|byte| *byte == 0)
            || self.amount == 0
            || self.memo.len() > PRIVATE_SETTLEMENT_MAX_AUDIT_MEMO_BYTES_V1
            || self.policy_references.len() > PRIVATE_SETTLEMENT_MAX_AUDIT_POLICY_REFERENCES_V1
            || self.inputs.len() != PRIVATE_SETTLEMENT_INPUT_SLOTS_V1
            || self.outputs.len() != PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1
        {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        if self.policy_references.iter().any(hash_is_zero)
            || self
                .policy_references
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        if !self.inputs[0].active || (self.inputs[1].active && !self.inputs[0].active) {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        for input in &self.inputs {
            input.validate()?;
        }
        self.payer_authorization.validate_shape()?;
        let authorized_nullifiers = self
            .payer_authorization
            .body
            .inputs
            .iter()
            .map(|input| input.nullifier)
            .collect::<Vec<_>>();
        if self.payer_authorization.body != self.payer_authorization_body(&authorized_nullifiers)? {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        let expected_roles = [
            PrivateSettlementAuditOutputRoleV1::SettlementRecipient,
            PrivateSettlementAuditOutputRoleV1::PayerChange,
            PrivateSettlementAuditOutputRoleV1::SponsorReimbursement,
        ];
        let mut view_keys = BTreeSet::new();
        for (index, (output, expected_role)) in self.outputs.iter().zip(expected_roles).enumerate()
        {
            if output.role != expected_role
                || output.recipient_view_key.iter().all(|byte| *byte == 0)
                || output
                    .encryption_opening
                    .ephemeral_secret
                    .iter()
                    .all(|byte| *byte == 0)
                || !view_keys.insert(output.recipient_view_key)
            {
                return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
            }
            output.view_key_authorization.validate_shape()?;
            if output.view_key_authorization.body
                != self.output_view_key_authorization_body(index)?
            {
                return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
            }
            output.note.validate()?;
        }
        if !self.outputs[0].note.active
            || self.outputs[0].note.value != self.amount
            || self.outputs[2].note.active != (self.sponsor_reimbursement_amount != 0)
            || self.outputs[2].note.value != self.sponsor_reimbursement_amount
        {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        let input_total = self
            .inputs
            .iter()
            .try_fold(0_u128, |total, input| total.checked_add(input.value));
        let output_total = self
            .outputs
            .iter()
            .try_fold(0_u128, |total, output| total.checked_add(output.note.value));
        if input_total.is_none() || input_total != output_total {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        let distinct_commitments = self
            .inputs
            .iter()
            .map(|input| input.commitment)
            .chain(self.outputs.iter().map(|output| output.note.commitment))
            .collect::<BTreeSet<_>>();
        if distinct_commitments.len()
            != PRIVATE_SETTLEMENT_INPUT_SLOTS_V1 + PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1
        {
            return Err(PrivateSettlementValidationError::InvalidAuditPlaintext);
        }
        Ok(())
    }

    /// Validate private business material against the exact public manifest leg.
    ///
    /// # Errors
    ///
    /// Returns a redacted binding error for any mismatch.
    pub fn validate_against_manifest(
        &self,
        manifest: &AtomicPrivateSettlementV1,
    ) -> Result<(), PrivateSettlementValidationError> {
        self.validate()?;
        manifest.validate()?;
        let leg = manifest
            .legs
            .get(usize::from(self.leg_ordinal))
            .ok_or(PrivateSettlementValidationError::UnknownLeg)?;
        let asset_binding = self
            .asset_binding_commitment()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if self.network_id != manifest.network_id
            || self.bundle_id != manifest.bundle_id
            || self.route != leg.route
            || self.pool_id != leg.pool_id
            || self.sponsor != manifest.sponsor
            || asset_binding != leg.asset_binding_commitment
            || self.fee_intent_digest != manifest.fee_intent_digest
            || self.settlement_expiry_height != manifest.expiry_height
            || (self.leg_ordinal == manifest.reimbursement_leg_ordinal)
                != (self.sponsor_reimbursement_amount != 0)
        {
            return Err(PrivateSettlementValidationError::AuditPlaintextBindingMismatch);
        }
        if self.leg_ordinal == manifest.reimbursement_leg_ordinal {
            let reimbursement_terms = self
                .reimbursement_terms_commitment()
                .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
            if reimbursement_terms != manifest.reimbursement_terms_commitment {
                return Err(PrivateSettlementValidationError::ReimbursementTermsMismatch);
            }
        }
        Ok(())
    }
}

/// Public authenticated-data fields for an encrypted audit capsule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditAadV1 {
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Stable settlement bundle identifier.
    pub bundle_id: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Exact route and incarnation.
    pub route: PrivateSettlementRouteV1,
    /// Digest of the exact four-validator committee roster and proofs of possession.
    pub authority_digest: Hash,
    /// Global/catalog height at which the exact committee authority is resolved.
    pub authority_context_height: u64,
    /// Digest of the governed policy.
    pub audit_policy_digest: Hash,
    /// Exact auditor key epoch.
    pub audit_key_epoch: u64,
    /// Commitment to the unpadded audit-capsule plaintext.
    pub plaintext_commitment: Hash,
}

/// Hybrid X25519 plus ML-KEM-768 public encryption key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementHybridPublicKeyV1 {
    /// X25519 public component.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub x25519: [u8; 32],
    /// Canonical ML-KEM-768 public key bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ml_kem_768: Vec<u8>,
}

impl PrivateSettlementHybridPublicKeyV1 {
    /// Construct the wire key from a validated hybrid public key.
    #[must_use]
    pub fn from_hybrid(key: &HybridPublicKey) -> Self {
        Self {
            x25519: key.x25519_bytes(),
            ml_kem_768: key.kyber_bytes().to_vec(),
        }
    }

    /// Parse and validate both public-key components.
    ///
    /// # Errors
    ///
    /// Returns a structural validation error for an invalid hybrid key.
    pub fn to_hybrid(&self) -> Result<HybridPublicKey, PrivateSettlementValidationError> {
        if self.ml_kem_768.len() != PRIVATE_SETTLEMENT_ML_KEM_768_PUBLIC_KEY_BYTES_V1 {
            return Err(PrivateSettlementValidationError::InvalidHybridPublicKey);
        }
        HybridPublicKey::from_bytes(self.x25519, &self.ml_kem_768)
            .map_err(|_| PrivateSettlementValidationError::InvalidHybridPublicKey)
    }

    /// Domain-separated digest used to detect duplicate encryption keys.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the key cannot be encoded.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(AUDITOR_KEY_DIGEST_DOMAIN_V1, self)
    }
}

/// One auditor authorized by a dataspace-local policy.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditorV1 {
    /// Stable auditor account identity.
    pub auditor_id: AccountId,
    /// Purpose-specific approval signing key.
    pub signing_key: PublicKey,
    /// Purpose-specific capsule encryption key.
    pub encryption_key: PrivateSettlementHybridPublicKeyV1,
}

impl fmt::Debug for PrivateSettlementAuditorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditorV1(<restricted>)")
    }
}

/// Self-authenticating body of one governed local auditor policy.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditPolicyBodyV1 {
    /// Policy wire version.
    pub version: u8,
    /// Dataspace governed by this policy.
    pub dataspace_id: DataSpaceId,
    /// Stable policy lineage identifier.
    pub policy_id: Hash,
    /// Monotonic non-zero policy revision.
    pub revision: u64,
    /// Monotonic non-zero encryption/signing key epoch.
    pub key_epoch: u64,
    /// First global height at which approvals are valid.
    pub activation_height: u64,
    /// Optional first height at which approvals are no longer valid.
    #[norito(required)]
    pub retirement_height: Option<u64>,
    /// Required distinct approval count; defaults operationally to one.
    pub min_approvals: u8,
    /// Strictly ordered authorized auditors.
    pub auditors: Vec<PrivateSettlementAuditorV1>,
}

impl fmt::Debug for PrivateSettlementAuditPolicyBodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditPolicyBodyV1(<restricted>)")
    }
}

impl PrivateSettlementAuditPolicyBodyV1 {
    /// Recompute the domain-separated digest of this exact policy body.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when the canonical policy body cannot be encoded.
    pub fn computed_policy_digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(AUDIT_POLICY_DIGEST_DOMAIN_V1, self)
    }
}

/// Governed auditor policy with a domain-separated self-digest.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditPolicyV1 {
    /// Exact policy body.
    pub body: PrivateSettlementAuditPolicyBodyV1,
    /// Domain-separated canonical body digest.
    pub policy_digest: Hash,
}

impl fmt::Debug for PrivateSettlementAuditPolicyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditPolicyV1(<restricted>)")
    }
}

impl PrivateSettlementAuditPolicyV1 {
    /// Construct a self-authenticating policy.
    ///
    /// # Errors
    ///
    /// Returns a typed validation error for malformed policy material.
    pub fn new(
        body: PrivateSettlementAuditPolicyBodyV1,
    ) -> Result<Self, PrivateSettlementValidationError> {
        let policy_digest = body
            .computed_policy_digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        let policy = Self {
            body,
            policy_digest,
        };
        policy.validate()?;
        Ok(policy)
    }

    /// Validate policy lifecycle, threshold, canonical auditors, keys, and self-digest.
    ///
    /// # Errors
    ///
    /// Returns a typed fail-closed policy error.
    pub fn validate(&self) -> Result<(), PrivateSettlementValidationError> {
        let body = &self.body;
        if body.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: body.version,
            });
        }
        if hash_is_zero(&body.policy_id)
            || body.revision == 0
            || body.key_epoch == 0
            || body.activation_height == 0
            || body
                .retirement_height
                .is_some_and(|height| height <= body.activation_height)
        {
            return Err(PrivateSettlementValidationError::InvalidAuditPolicyLifecycle);
        }
        if body.auditors.is_empty()
            || body.auditors.len() > PRIVATE_SETTLEMENT_MAX_AUDITORS_V1
            || body.min_approvals == 0
            || usize::from(body.min_approvals) > body.auditors.len()
        {
            return Err(PrivateSettlementValidationError::InvalidAuditThreshold);
        }
        let mut previous = None;
        let mut signing_keys = BTreeSet::new();
        let mut encryption_keys = BTreeSet::new();
        for auditor in &body.auditors {
            if previous
                .as_ref()
                .is_some_and(|value| value >= &auditor.auditor_id)
            {
                return Err(PrivateSettlementValidationError::NonCanonicalAuditorOrder);
            }
            previous = Some(auditor.auditor_id.clone());
            auditor.encryption_key.to_hybrid()?;
            let encryption_digest = auditor
                .encryption_key
                .digest()
                .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
            if !signing_keys.insert(auditor.signing_key.clone())
                || !encryption_keys.insert(encryption_digest)
            {
                return Err(PrivateSettlementValidationError::DuplicateAuditorKey);
            }
        }
        let expected = body
            .computed_policy_digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if expected != self.policy_digest {
            return Err(PrivateSettlementValidationError::AuditPolicyDigestMismatch);
        }
        Ok(())
    }

    /// Return whether the policy is active at a global block height.
    #[must_use]
    pub fn is_active_at(&self, height: u64) -> bool {
        height >= self.body.activation_height
            && self
                .body
                .retirement_height
                .is_none_or(|retirement| height < retirement)
    }

    /// Recompute the domain-separated digest of this policy's exact body.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when the canonical policy body cannot be encoded.
    pub fn computed_policy_digest(&self) -> Result<Hash, norito::Error> {
        self.body.computed_policy_digest()
    }
}

/// Canonical activation interval and revision of one restricted pool mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementPoolGovernanceLifecycleV1 {
    /// Monotonic non-zero revision within this opaque pool's governance lineage.
    pub governance_revision: u64,
    /// First global height at which this exact mapping may be used.
    pub activation_height: u64,
    /// Optional first global height at which this exact mapping is retired.
    #[norito(required)]
    pub retirement_height: Option<u64>,
}

impl PrivateSettlementPoolGovernanceLifecycleV1 {
    fn validate(self) -> Result<(), PrivateSettlementValidationError> {
        if self.governance_revision == 0
            || self.activation_height == 0
            || self
                .retirement_height
                .is_some_and(|retirement| retirement <= self.activation_height)
        {
            return Err(PrivateSettlementValidationError::InvalidPoolGovernanceLifecycle);
        }
        Ok(())
    }

    /// Return whether the exact governance revision is active at `height`.
    #[must_use]
    pub fn is_active_at(self, height: u64) -> bool {
        height >= self.activation_height
            && self
                .retirement_height
                .is_none_or(|retirement| height < retirement)
    }
}

/// Restricted body mapping one opaque settlement pool to one exact asset.
///
/// This body contains the literal asset identifier and its random commitment
/// opening. It belongs only in access-controlled governance state and auditor
/// capsules; public manifests and receipts carry only `pool_id`,
/// `asset_binding_commitment`, and `audit_policy_digest`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementPoolGovernanceBodyV1 {
    /// Wire version; must be [`ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1`].
    pub version: u8,
    /// Exact dataspace, lane, and active incarnation hosting this pool.
    pub route: PrivateSettlementRouteV1,
    /// Singular opaque private-note pool identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact restricted asset definition mapped to `pool_id`.
    pub asset_definition_id: AssetDefinitionId,
    /// Random non-zero opening salt for `asset_binding_commitment`.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub asset_binding_salt: [u8; 32],
    /// Domain-separated commitment to the complete route, pool, asset, and salt.
    pub asset_binding_commitment: Hash,
    /// Digest of the exact governed local audit policy.
    pub audit_policy_digest: Hash,
    /// Exact non-zero signing/encryption epoch of that audit policy.
    pub audit_key_epoch: u64,
    /// Activation interval and monotonic revision of this mapping.
    pub lifecycle: PrivateSettlementPoolGovernanceLifecycleV1,
}

impl fmt::Debug for PrivateSettlementPoolGovernanceBodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementPoolGovernanceBodyV1(<restricted>)")
    }
}

impl PrivateSettlementPoolGovernanceBodyV1 {
    /// Construct a restricted mapping while deriving its asset and policy commitments.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the route, policy, lifecycle, pool, or salt is invalid.
    pub fn new(
        route: PrivateSettlementRouteV1,
        pool_id: PrivacyPoolIdV1,
        asset_definition_id: AssetDefinitionId,
        asset_binding_salt: [u8; 32],
        policy: &PrivateSettlementAuditPolicyV1,
        lifecycle: PrivateSettlementPoolGovernanceLifecycleV1,
    ) -> Result<Self, PrivateSettlementValidationError> {
        policy.validate()?;
        let asset_binding_commitment = private_settlement_asset_binding_commitment_v1(
            route,
            pool_id,
            &asset_definition_id,
            asset_binding_salt,
        )
        .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        let audit_policy_digest = policy
            .computed_policy_digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        let body = Self {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            route,
            pool_id,
            asset_definition_id,
            asset_binding_salt,
            asset_binding_commitment,
            audit_policy_digest,
            audit_key_epoch: policy.body.key_epoch,
            lifecycle,
        };
        body.validate()?;
        if body.route.dataspace_id != policy.body.dataspace_id {
            return Err(PrivateSettlementValidationError::PoolGovernancePolicyMismatch);
        }
        Ok(body)
    }

    /// Recompute the domain-separated asset-binding commitment from its opening.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when the exact opening cannot be canonically encoded.
    pub fn computed_asset_binding_commitment(&self) -> Result<Hash, norito::Error> {
        private_settlement_asset_binding_commitment_v1(
            self.route,
            self.pool_id,
            &self.asset_definition_id,
            self.asset_binding_salt,
        )
    }

    fn validate(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if self.route.dataspace_id == DataSpaceId::UNIVERSAL
            || hash_is_zero(&self.route.lane_incarnation)
        {
            return Err(PrivateSettlementValidationError::InvalidPoolGovernanceRoute);
        }
        self.lifecycle.validate()?;
        if self.pool_id.is_zero()
            || self.asset_binding_salt.iter().all(|byte| *byte == 0)
            || hash_is_zero(&self.asset_binding_commitment)
            || hash_is_zero(&self.audit_policy_digest)
            || self.audit_key_epoch == 0
        {
            return Err(PrivateSettlementValidationError::InvalidPoolGovernanceBinding);
        }
        let expected = self
            .computed_asset_binding_commitment()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if expected != self.asset_binding_commitment {
            return Err(PrivateSettlementValidationError::PoolGovernanceAssetBindingMismatch);
        }
        Ok(())
    }
}

/// Self-authenticating restricted governance record for one confidential pool.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementPoolGovernanceV1 {
    /// Exact restricted pool mapping and lifecycle.
    pub body: PrivateSettlementPoolGovernanceBodyV1,
    /// Domain-separated canonical self-digest of `body`.
    pub governance_digest: Hash,
}

impl fmt::Debug for PrivateSettlementPoolGovernanceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementPoolGovernanceV1(<restricted>)")
    }
}

impl PrivateSettlementPoolGovernanceV1 {
    /// Construct a self-authenticating restricted governance record.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the body or its canonical encoding is invalid.
    pub fn new(
        body: PrivateSettlementPoolGovernanceBodyV1,
    ) -> Result<Self, PrivateSettlementValidationError> {
        let governance_digest = canonical_hash(POOL_GOVERNANCE_DIGEST_DOMAIN_V1, &body)
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        let record = Self {
            body,
            governance_digest,
        };
        record.validate()?;
        Ok(record)
    }

    /// Construct a self-authenticating record from one exact restricted mapping.
    ///
    /// # Errors
    ///
    /// Returns a typed error when any binding, policy, or lifecycle field is invalid.
    pub fn from_restricted_mapping(
        route: PrivateSettlementRouteV1,
        pool_id: PrivacyPoolIdV1,
        asset_definition_id: AssetDefinitionId,
        asset_binding_salt: [u8; 32],
        policy: &PrivateSettlementAuditPolicyV1,
        lifecycle: PrivateSettlementPoolGovernanceLifecycleV1,
    ) -> Result<Self, PrivateSettlementValidationError> {
        let activation_height = lifecycle.activation_height;
        let record = Self::new(PrivateSettlementPoolGovernanceBodyV1::new(
            route,
            pool_id,
            asset_definition_id,
            asset_binding_salt,
            policy,
            lifecycle,
        )?)?;
        record.validate_against_policy_at(policy, activation_height)?;
        Ok(record)
    }

    /// Recompute this record's domain-separated canonical self-digest.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when the exact body cannot be canonically encoded.
    pub fn computed_governance_digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(POOL_GOVERNANCE_DIGEST_DOMAIN_V1, &self.body)
    }

    /// Validate all structural bindings and the canonical self-digest.
    ///
    /// # Errors
    ///
    /// Returns a typed fail-closed governance validation error.
    pub fn validate(&self) -> Result<(), PrivateSettlementValidationError> {
        self.body.validate()?;
        let expected = self
            .computed_governance_digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if hash_is_zero(&self.governance_digest) || expected != self.governance_digest {
            return Err(PrivateSettlementValidationError::PoolGovernanceDigestMismatch);
        }
        Ok(())
    }

    /// Verify an exact restricted route, pool, asset, and salt opening.
    ///
    /// The error deliberately does not reveal which restricted component was
    /// wrong so callers can keep denial responses uniform.
    ///
    /// # Errors
    ///
    /// Returns a typed mismatch when any opening component differs.
    pub fn validate_asset_opening(
        &self,
        route: PrivateSettlementRouteV1,
        pool_id: PrivacyPoolIdV1,
        asset_definition_id: &AssetDefinitionId,
        asset_binding_salt: [u8; 32],
    ) -> Result<(), PrivateSettlementValidationError> {
        self.validate()?;
        if self.body.route != route
            || self.body.pool_id != pool_id
            || &self.body.asset_definition_id != asset_definition_id
            || self.body.asset_binding_salt != asset_binding_salt
        {
            return Err(PrivateSettlementValidationError::PoolGovernanceAssetBindingMismatch);
        }
        let expected = private_settlement_asset_binding_commitment_v1(
            route,
            pool_id,
            asset_definition_id,
            asset_binding_salt,
        )
        .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if expected != self.body.asset_binding_commitment {
            return Err(PrivateSettlementValidationError::PoolGovernanceAssetBindingMismatch);
        }
        Ok(())
    }

    /// Validate the exact audit policy, key epoch, lifecycle coverage, and height.
    ///
    /// # Errors
    ///
    /// Returns a typed mismatch for the wrong policy or epoch, an invalid
    /// lifecycle error when the mapping outlives its policy, or a stale error
    /// when either record is inactive at `height`.
    pub fn validate_against_policy_at(
        &self,
        policy: &PrivateSettlementAuditPolicyV1,
        height: u64,
    ) -> Result<(), PrivateSettlementValidationError> {
        self.validate()?;
        policy.validate()?;
        let expected_policy_digest = policy
            .computed_policy_digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if self.body.route.dataspace_id != policy.body.dataspace_id
            || self.body.audit_policy_digest != expected_policy_digest
            || self.body.audit_policy_digest != policy.policy_digest
            || self.body.audit_key_epoch != policy.body.key_epoch
        {
            return Err(PrivateSettlementValidationError::PoolGovernancePolicyMismatch);
        }
        if self.body.lifecycle.activation_height < policy.body.activation_height
            || match policy.body.retirement_height {
                Some(policy_retirement) => self
                    .body
                    .lifecycle
                    .retirement_height
                    .is_none_or(|retirement| retirement > policy_retirement),
                None => false,
            }
        {
            return Err(PrivateSettlementValidationError::InvalidPoolGovernanceLifecycle);
        }
        if !self.body.lifecycle.is_active_at(height) || !policy.is_active_at(height) {
            return Err(PrivateSettlementValidationError::StalePoolGovernance);
        }
        Ok(())
    }

    /// Return whether this exact mapping revision is active at `height`.
    #[must_use]
    pub fn is_active_at(&self, height: u64) -> bool {
        self.body.lifecycle.is_active_at(height)
    }
}

/// One hybrid KEM-wrapped data-encryption key addressed to an auditor.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementWrappedDekV1 {
    /// Auditor that may unwrap this DEK.
    pub auditor_id: AccountId,
    /// Sender's ephemeral X25519 public component.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub ephemeral_x25519: [u8; 32],
    /// ML-KEM-768 encapsulation ciphertext.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ml_kem_ciphertext: Vec<u8>,
    /// XChaCha20-Poly1305 nonce used to wrap the DEK.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub nonce: [u8; PRIVATE_SETTLEMENT_XCHACHA_NONCE_BYTES_V1],
    /// Encrypted 32-byte DEK and authentication tag.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub wrapped_dek: Vec<u8>,
}

impl fmt::Debug for PrivateSettlementWrappedDekV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementWrappedDekV1(<redacted>)")
    }
}

/// Padded encrypted auditor capsule and independently wrapped DEKs.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditCapsuleV1 {
    /// Capsule wire version.
    pub version: u8,
    /// Exact public authenticated data.
    pub aad: PrivateSettlementAuditAadV1,
    /// Fixed padding class.
    pub padding: PrivateSettlementCapsulePaddingV1,
    /// XChaCha20-Poly1305 nonce used for the capsule payload.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub nonce: [u8; PRIVATE_SETTLEMENT_XCHACHA_NONCE_BYTES_V1],
    /// Padded encrypted capsule and authentication tag.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ciphertext: Vec<u8>,
    /// Strictly ordered independently wrapped DEKs.
    pub wrapped_deks: Vec<PrivateSettlementWrappedDekV1>,
}

impl fmt::Debug for PrivateSettlementAuditCapsuleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditCapsuleV1(<redacted>)")
    }
}

impl PrivateSettlementAuditCapsuleV1 {
    /// Compute the digest bound by the proof statement and auditor approval.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the capsule cannot be encoded.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(CAPSULE_DIGEST_DOMAIN_V1, self)
    }

    /// Validate padding, authenticated-data fields, and recipient wrapping shape.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a malformed or policy-inconsistent capsule.
    pub fn validate_against(
        &self,
        policy: &PrivateSettlementAuditPolicyV1,
    ) -> Result<(), PrivateSettlementValidationError> {
        policy.validate()?;
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if self.aad.route.dataspace_id != policy.body.dataspace_id
            || self.aad.audit_policy_digest != policy.policy_digest
            || self.aad.audit_key_epoch != policy.body.key_epoch
            || self.aad.network_id.as_bytes().iter().all(|byte| *byte == 0)
            || hash_is_zero(&self.aad.bundle_id)
            || hash_is_zero(&self.aad.route.lane_incarnation)
            || hash_is_zero(&self.aad.authority_digest)
            || self.aad.authority_context_height == 0
            || hash_is_zero(&self.aad.plaintext_commitment)
        {
            return Err(PrivateSettlementValidationError::AuditCapsuleBindingMismatch);
        }
        if self.nonce.iter().all(|byte| *byte == 0)
            || self.ciphertext.len() != self.padding.ciphertext_bytes()
            || self.ciphertext.iter().all(|byte| *byte == 0)
        {
            return Err(PrivateSettlementValidationError::InvalidAuditCapsuleCiphertext);
        }
        if self.wrapped_deks.len() != policy.body.auditors.len() {
            return Err(PrivateSettlementValidationError::AuditCapsuleRecipientMismatch);
        }
        for (wrapped, auditor) in self.wrapped_deks.iter().zip(&policy.body.auditors) {
            if wrapped.auditor_id != auditor.auditor_id
                || wrapped.ephemeral_x25519.iter().all(|byte| *byte == 0)
                || wrapped.ml_kem_ciphertext.len()
                    != PRIVATE_SETTLEMENT_ML_KEM_768_CIPHERTEXT_BYTES_V1
                || wrapped.ml_kem_ciphertext.iter().all(|byte| *byte == 0)
                || wrapped.nonce.iter().all(|byte| *byte == 0)
                || wrapped.wrapped_dek.len() != PRIVATE_SETTLEMENT_WRAPPED_DEK_BYTES_V1
                || wrapped.wrapped_dek.iter().all(|byte| *byte == 0)
            {
                return Err(PrivateSettlementValidationError::InvalidWrappedDek);
            }
        }
        Ok(())
    }
}

/// Exact restricted-DA statement certified by one participant committee.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementSidecarAvailabilityBodyV1 {
    /// Certificate body wire version.
    pub version: u8,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Stable bundle identifier.
    pub bundle_id: Hash,
    /// Canonical participant-leg ordinal.
    pub leg_ordinal: u8,
    /// Exact participant route and lane incarnation.
    pub route: PrivateSettlementRouteV1,
    /// Digest of the exact four-validator committee authority.
    pub authority_digest: Hash,
    /// Global/catalog height used to resolve the committee.
    pub authority_context_height: u64,
    /// Digest of the complete sidecar material excluding this ticket.
    pub payload_digest: Hash,
    /// Canonical byte length of sidecar material excluding this certificate.
    pub payload_bytes: u32,
    /// Height through which restricted DA must retain the sidecar.
    pub retention_until_height: u64,
}

impl PrivateSettlementSidecarAvailabilityBodyV1 {
    /// Canonical purpose-separated bytes signed by each availability validator.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the body cannot be canonically encoded.
    pub fn signature_preimage(&self) -> Result<Vec<u8>, norito::Error> {
        let body = norito::encode_canonical(self)?;
        let body_len = u64::try_from(body.len()).map_err(|_| {
            norito::Error::Io(std::io::Error::other(
                "availability certificate body is too large",
            ))
        })?;
        let mut preimage = Vec::with_capacity(
            SIDECAR_AVAILABILITY_SIGNATURE_DOMAIN_V1.len()
                + std::mem::size_of::<u64>()
                + body.len(),
        );
        preimage.extend_from_slice(SIDECAR_AVAILABILITY_SIGNATURE_DOMAIN_V1);
        preimage.extend_from_slice(&body_len.to_le_bytes());
        preimage.extend_from_slice(&body);
        Ok(preimage)
    }

    /// Validate the immutable availability statement before any signature work.
    ///
    /// # Errors
    ///
    /// Returns a typed error for reserved fields or invalid height bounds.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if self.network_id.as_bytes().iter().all(|byte| *byte == 0)
            || hash_is_zero(&self.bundle_id)
            || hash_is_zero(&self.route.lane_incarnation)
            || hash_is_zero(&self.authority_digest)
            || hash_is_zero(&self.payload_digest)
            || self.authority_context_height == 0
            || self.payload_bytes == 0
            || self.retention_until_height <= self.authority_context_height
        {
            return Err(PrivateSettlementValidationError::InvalidAvailabilityCertificate);
        }
        Ok(())
    }
}

/// One independently authenticated availability share from a committee validator.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAvailabilityShareV1 {
    /// Share wire version.
    pub version: u8,
    /// Exact immutable body signed by the validator.
    pub body: PrivateSettlementSidecarAvailabilityBodyV1,
    /// Exact committee identity that produced the share.
    pub signer: PeerId,
    /// Compressed BLS-normal signature over [`Self::body`].
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub signature: Vec<u8>,
}

impl PrivateSettlementAvailabilityShareV1 {
    /// Validate fixed wire shape before committee membership and cryptography.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a malformed share.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        self.body.validate_shape()?;
        if self.signature.len() != PRIVATE_SETTLEMENT_BLS_BYTES_V1 {
            return Err(PrivateSettlementValidationError::InvalidAvailabilityShare);
        }
        Ok(())
    }
}

/// Signed restricted-DA availability certificate carried with one leg.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementSidecarAvailabilityV1 {
    /// Exact same-message body certified by the committee.
    pub body: PrivateSettlementSidecarAvailabilityBodyV1,
    /// Exact three-of-four LSB-first signer bitmap.
    pub signers_bitmap: u8,
    /// Compressed aggregate BLS-normal signature over the canonical body.
    pub aggregate_signature: Vec<u8>,
}

impl PrivateSettlementSidecarAvailabilityV1 {
    /// Compute the manifest-committed certificate digest.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the certificate cannot be encoded.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(
            b"iroha:nexus:private-settlement:availability-certificate:v1\0",
            self,
        )
    }

    /// Canonical same-message bytes signed by the availability quorum.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the body cannot be encoded.
    pub fn signature_preimage(&self) -> Result<Vec<u8>, norito::Error> {
        self.body.signature_preimage()
    }

    /// Validate certificate cardinality and fixed wire shape before cryptography.
    ///
    /// # Errors
    ///
    /// Returns a typed structural error for malformed or zero fields.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        self.body.validate_shape()?;
        if self.signers_bitmap & !0x0f != 0
            || self.signers_bitmap.count_ones() != u32::from(PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1)
            || self.aggregate_signature.len() != PRIVATE_SETTLEMENT_BLS_BYTES_V1
        {
            return Err(PrivateSettlementValidationError::InvalidAvailabilityCertificate);
        }
        Ok(())
    }
}

/// Exact restricted auditor view committed by a node response attestation.
///
/// This material is not a second HTTP shape. It is the canonical, typed input
/// to [`Self::digest`], so implementations cannot silently omit a response
/// field when authenticating an auditor view.
#[derive(Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct PrivateSettlementAuditorViewDigestMaterialV1 {
    /// Digest-material wire version.
    pub version: u8,
    /// Node-authoritative height used for access and policy evaluation.
    pub authoritative_height: u64,
    /// Exact public bundle manifest.
    pub manifest: AtomicPrivateSettlementV1,
    /// Exact historical governed policy bound by the encrypted sidecar.
    pub audit_policy: PrivateSettlementAuditPolicyV1,
    /// Exact current policy used to authorize restricted access.
    ///
    /// This can equal `audit_policy` or be a later policy in the same governed
    /// lineage when retained historical capsule material is read after key
    /// rotation.
    pub access_audit_policy: PrivateSettlementAuditPolicyV1,
    /// Exact four-validator participant authority.
    pub committee_authority: PrivateSettlementCommitteeAuthorityV1,
    /// Restricted proof statement; proof bytes remain absent.
    pub statement: PrivateSettlementProofStatementV1,
    /// Opaque fixed-shape private-state transition.
    pub delta: PrivateSettlementDeltaV1,
    /// Padded hybrid-encrypted auditor capsule.
    pub audit_capsule: PrivateSettlementAuditCapsuleV1,
    /// Durable restricted-DA certificate.
    pub availability: PrivateSettlementSidecarAvailabilityV1,
    /// Explicit stable code for the durable lifecycle projected by Torii.
    pub lifecycle_code: u8,
}

impl fmt::Debug for PrivateSettlementAuditorViewDigestMaterialV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateSettlementAuditorViewDigestMaterialV1")
            .field("bundle_id", &self.manifest.bundle_id)
            .field("leg_ordinal", &self.statement.leg_ordinal)
            .field("route", &self.statement.route)
            .field("authoritative_height", &self.authoritative_height)
            .field("lifecycle_code", &self.lifecycle_code)
            .finish_non_exhaustive()
    }
}

impl PrivateSettlementAuditorViewDigestMaterialV1 {
    /// Compute the purpose-separated digest of the complete restricted view.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the exact typed view cannot be encoded.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(AUDITOR_VIEW_DIGEST_DOMAIN_V1, self)
    }
}

/// Exact node-authenticated statement over one restricted auditor view.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditorViewAttestationBodyV1 {
    /// Attestation wire version.
    pub version: u8,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Content address of the exact encrypted participant leg.
    pub payload_digest: Hash,
    /// Digest of every unsigned auditor-response field.
    pub view_digest: Hash,
    /// Digest of the exact four-validator authority.
    pub authority_digest: Hash,
    /// Stable code for the exact lifecycle included in the view digest.
    pub lifecycle_code: u8,
    /// Exact height included in the view digest.
    pub authoritative_height: u64,
    /// Committee validator that served and authenticated the view.
    pub responder: PeerId,
}

impl PrivateSettlementAuditorViewAttestationBodyV1 {
    /// Canonical purpose-separated bytes signed by the responding validator.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the body cannot be canonically encoded.
    pub fn signature_preimage(&self) -> Result<Vec<u8>, norito::Error> {
        let body = norito::encode_canonical(self)?;
        let body_len = u64::try_from(body.len()).map_err(|_| {
            norito::Error::Io(std::io::Error::other(
                "auditor view attestation body is too large",
            ))
        })?;
        let mut preimage = Vec::with_capacity(
            AUDITOR_VIEW_ATTESTATION_SIGNATURE_DOMAIN_V1.len()
                + std::mem::size_of::<u64>()
                + body.len(),
        );
        preimage.extend_from_slice(AUDITOR_VIEW_ATTESTATION_SIGNATURE_DOMAIN_V1);
        preimage.extend_from_slice(&body_len.to_le_bytes());
        preimage.extend_from_slice(&body);
        Ok(preimage)
    }

    /// Validate fixed attestation fields before authority and signature work.
    ///
    /// # Errors
    ///
    /// Returns a typed error for reserved values or an unknown lifecycle code.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if self.network_id.as_bytes().iter().all(|byte| *byte == 0)
            || hash_is_zero(&self.payload_digest)
            || hash_is_zero(&self.view_digest)
            || hash_is_zero(&self.authority_digest)
            || self.authoritative_height == 0
            || self.lifecycle_code > PRIVATE_SETTLEMENT_LIFECYCLE_EXPIRED_V1
        {
            return Err(PrivateSettlementValidationError::InvalidAuditorViewAttestation);
        }
        Ok(())
    }
}

/// One committee validator's BLS authentication of an auditor capsule view.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditorViewAttestationV1 {
    /// Exact purpose-separated body signed by the responder.
    pub body: PrivateSettlementAuditorViewAttestationBodyV1,
    /// Compressed BLS-normal signature over [`Self::body`].
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub signature: Vec<u8>,
}

impl PrivateSettlementAuditorViewAttestationV1 {
    /// Validate fixed wire shape before roster and cryptographic verification.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed body or signature bytes.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        self.body.validate_shape()?;
        if self.signature.len() != PRIVATE_SETTLEMENT_BLS_BYTES_V1 {
            return Err(PrivateSettlementValidationError::InvalidAuditorViewAttestation);
        }
        Ok(())
    }
}

/// Exact approval-acknowledgement view committed by a node attestation.
#[derive(Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct PrivateSettlementAuditApprovalAcknowledgementDigestMaterialV1 {
    /// Digest-material wire version.
    pub version: u8,
    /// Node-authoritative height at which the approval became durable.
    pub authoritative_height: u64,
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the encrypted leg.
    pub payload_digest: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Exact four-validator participant authority returned by the node.
    pub committee_authority: PrivateSettlementCommitteeAuthorityV1,
    /// Number of distinct governed approvals now durable.
    pub collected: u8,
    /// Governed approval threshold.
    pub required: u8,
    /// Whether this request inserted new durable approval material.
    pub newly_recorded: bool,
    /// Stable code for the exact durable lifecycle returned by the node.
    pub lifecycle_code: u8,
}

impl fmt::Debug for PrivateSettlementAuditApprovalAcknowledgementDigestMaterialV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateSettlementAuditApprovalAcknowledgementDigestMaterialV1")
            .field("bundle_id", &self.bundle_id)
            .field("payload_digest", &self.payload_digest)
            .field("leg_ordinal", &self.leg_ordinal)
            .field("authoritative_height", &self.authoritative_height)
            .field("collected", &self.collected)
            .field("required", &self.required)
            .field("newly_recorded", &self.newly_recorded)
            .field("lifecycle_code", &self.lifecycle_code)
            .finish_non_exhaustive()
    }
}

impl PrivateSettlementAuditApprovalAcknowledgementDigestMaterialV1 {
    /// Compute the purpose-separated digest of the complete acknowledgement.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the exact typed view cannot be encoded.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(AUDIT_APPROVAL_ACKNOWLEDGEMENT_DIGEST_DOMAIN_V1, self)
    }
}

/// Exact node-authenticated statement over one durable approval acknowledgement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1 {
    /// Attestation wire version.
    pub version: u8,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Content address of the exact encrypted participant leg.
    pub payload_digest: Hash,
    /// Digest of the exact signed auditor approval request.
    pub approval_digest: Hash,
    /// Digest of every unsigned acknowledgement-response field.
    pub acknowledgement_digest: Hash,
    /// Digest of the exact four-validator authority.
    pub authority_digest: Hash,
    /// Stable code for the exact lifecycle included in the acknowledgement.
    pub lifecycle_code: u8,
    /// Exact height included in the acknowledgement.
    pub authoritative_height: u64,
    /// Committee validator that persisted and authenticated the approval.
    pub responder: PeerId,
}

impl PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1 {
    /// Canonical purpose-separated bytes signed by the responding validator.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the body cannot be canonically encoded.
    pub fn signature_preimage(&self) -> Result<Vec<u8>, norito::Error> {
        let body = norito::encode_canonical(self)?;
        let body_len = u64::try_from(body.len()).map_err(|_| {
            norito::Error::Io(std::io::Error::other(
                "audit approval acknowledgement attestation body is too large",
            ))
        })?;
        let mut preimage = Vec::with_capacity(
            AUDIT_APPROVAL_ACKNOWLEDGEMENT_ATTESTATION_SIGNATURE_DOMAIN_V1.len()
                + std::mem::size_of::<u64>()
                + body.len(),
        );
        preimage.extend_from_slice(AUDIT_APPROVAL_ACKNOWLEDGEMENT_ATTESTATION_SIGNATURE_DOMAIN_V1);
        preimage.extend_from_slice(&body_len.to_le_bytes());
        preimage.extend_from_slice(&body);
        Ok(preimage)
    }

    /// Validate fixed fields before authority and signature verification.
    ///
    /// # Errors
    ///
    /// Returns a typed error for reserved values or an invalid lifecycle.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if self.network_id.as_bytes().iter().all(|byte| *byte == 0)
            || hash_is_zero(&self.payload_digest)
            || hash_is_zero(&self.approval_digest)
            || hash_is_zero(&self.acknowledgement_digest)
            || hash_is_zero(&self.authority_digest)
            || self.authoritative_height == 0
            || self.lifecycle_code > PRIVATE_SETTLEMENT_LIFECYCLE_AUDITED_V1
        {
            return Err(
                PrivateSettlementValidationError::InvalidAuditApprovalAcknowledgementAttestation,
            );
        }
        Ok(())
    }
}

/// One committee validator's BLS authentication of an approval acknowledgement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditApprovalAcknowledgementAttestationV1 {
    /// Exact purpose-separated body signed by the responder.
    pub body: PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1,
    /// Compressed BLS-normal signature over [`Self::body`].
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub signature: Vec<u8>,
}

impl PrivateSettlementAuditApprovalAcknowledgementAttestationV1 {
    /// Validate fixed wire shape before roster and cryptographic verification.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed body or signature bytes.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        self.body.validate_shape()?;
        if self.signature.len() != PRIVATE_SETTLEMENT_BLS_BYTES_V1 {
            return Err(
                PrivateSettlementValidationError::InvalidAuditApprovalAcknowledgementAttestation,
            );
        }
        Ok(())
    }
}

/// Complete restricted sidecar verified by one participant committee.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementLegPayloadV1 {
    /// Restricted proof statement.
    pub statement: PrivateSettlementProofStatementV1,
    /// Native proof bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub proof: Vec<u8>,
    /// Fixed-shape state delta.
    pub delta: PrivateSettlementDeltaV1,
    /// Padded auditor-only capsule.
    pub audit_capsule: PrivateSettlementAuditCapsuleV1,
    /// Restricted-DA availability metadata.
    pub availability: PrivateSettlementSidecarAvailabilityV1,
}

impl fmt::Debug for PrivateSettlementLegPayloadV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateSettlementLegPayloadV1")
            .field("bundle_id", &self.statement.bundle_id)
            .field("leg_ordinal", &self.statement.leg_ordinal)
            .field("route", &self.statement.route)
            .finish_non_exhaustive()
    }
}

impl PrivateSettlementLegPayloadV1 {
    /// Compute the proof-byte digest committed by the delta.
    #[must_use]
    pub fn proof_digest(&self) -> Hash {
        private_settlement_proof_digest_v1(&self.proof)
    }

    fn sidecar_material_digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(
            SIDECAR_DIGEST_DOMAIN_V1,
            &(
                self.statement.clone(),
                self.proof.clone(),
                self.delta.clone(),
                self.audit_capsule.clone(),
            ),
        )
    }

    /// Compute the sidecar digest committed by the public manifest.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if sidecar material cannot be encoded.
    pub fn payload_digest(&self) -> Result<Hash, norito::Error> {
        self.sidecar_material_digest()
    }

    /// Return the canonical byte length of the complete sidecar, including its ticket.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the sidecar cannot be encoded.
    pub fn canonical_bytes_len(&self) -> Result<usize, norito::Error> {
        norito::encode_canonical(self).map(|encoded| encoded.len())
    }

    /// Return the canonical sidecar-material byte length excluding its certificate.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error when material cannot be encoded.
    pub fn sidecar_material_bytes_len(&self) -> Result<usize, norito::Error> {
        norito::encode_canonical(&(
            self.statement.clone(),
            self.proof.clone(),
            self.delta.clone(),
            self.audit_capsule.clone(),
        ))
        .map(|encoded| encoded.len())
    }

    /// Validate all cross-object bindings before proof verification or staging.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed, substituted, stale, or unavailable material.
    pub fn validate_against(
        &self,
        manifest: &AtomicPrivateSettlementV1,
        policy: &PrivateSettlementAuditPolicyV1,
    ) -> Result<(), PrivateSettlementValidationError> {
        let (delta_digest, payload_digest, canonical_payload_bytes) =
            self.validate_material_against_v1(manifest, policy, false)?;
        let leg = manifest
            .legs
            .get(usize::from(self.statement.leg_ordinal))
            .ok_or(PrivateSettlementValidationError::UnknownLeg)?;
        self.availability.validate_shape()?;
        let certificate_digest = self
            .availability
            .digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        let availability = &self.availability.body;
        if leg.delta_digest != delta_digest
            || leg.payload_digest != payload_digest
            || leg.availability_certificate_digest != certificate_digest
            || availability.network_id != manifest.network_id
            || availability.bundle_id != manifest.bundle_id
            || availability.leg_ordinal != self.statement.leg_ordinal
            || availability.route != self.statement.route
            || availability.authority_digest != self.audit_capsule.aad.authority_digest
            || availability.authority_context_height != manifest.authority_context_height
            || availability.payload_digest != payload_digest
            || usize::try_from(availability.payload_bytes).ok() != Some(canonical_payload_bytes)
            || availability.retention_until_height < manifest.expiry_height
        {
            return Err(PrivateSettlementValidationError::SidecarAvailabilityMismatch);
        }
        Ok(())
    }

    fn validate_material_against_v1(
        &self,
        manifest: &AtomicPrivateSettlementV1,
        policy: &PrivateSettlementAuditPolicyV1,
        provisional: bool,
    ) -> Result<(Hash, Hash, usize), PrivateSettlementValidationError> {
        if provisional {
            manifest.validate_provisional()?;
        } else {
            manifest.validate()?;
        }
        policy.validate()?;
        if self.proof.is_empty() || self.proof.len() > PRIVATE_SETTLEMENT_MAX_PROOF_BYTES_V1 {
            return Err(PrivateSettlementValidationError::InvalidProofSize);
        }
        self.statement.validate()?;
        let leg = manifest
            .legs
            .get(usize::from(self.statement.leg_ordinal))
            .ok_or(PrivateSettlementValidationError::UnknownLeg)?;
        if self.statement.network_id != manifest.network_id
            || self.statement.bundle_id != manifest.bundle_id
            || self.statement.route != leg.route
            || self.statement.authority_context_height != manifest.authority_context_height
            || self.statement.pool_id != leg.pool_id
            || self.statement.asset_binding_commitment != leg.asset_binding_commitment
            || self.statement.audit_policy_digest != leg.audit_policy_digest
            || self.statement.fee_intent_digest != manifest.fee_intent_digest
            || self.statement.reimbursement_terms_commitment
                != manifest.reimbursement_terms_commitment
            || self.statement.reimbursement_leg_ordinal != manifest.reimbursement_leg_ordinal
            || self.statement.expiry_height != manifest.expiry_height
            || policy.body.dataspace_id != leg.route.dataspace_id
            || policy.policy_digest != leg.audit_policy_digest
            || policy.body.key_epoch != self.statement.audit_key_epoch
            || !policy.is_active_at(manifest.authority_context_height)
            || policy
                .body
                .retirement_height
                .is_some_and(|retirement| manifest.expiry_height >= retirement)
        {
            return Err(PrivateSettlementValidationError::ManifestPayloadMismatch);
        }
        self.audit_capsule.validate_against(policy)?;
        if self.audit_capsule.aad.network_id != self.statement.network_id
            || self.audit_capsule.aad.bundle_id != self.statement.bundle_id
            || self.audit_capsule.aad.leg_ordinal != self.statement.leg_ordinal
            || self.audit_capsule.aad.route != self.statement.route
            || self.audit_capsule.aad.authority_context_height
                != self.statement.authority_context_height
            || self.audit_capsule.aad.plaintext_commitment
                != self.statement.audit_plaintext_commitment
        {
            return Err(PrivateSettlementValidationError::AuditCapsuleBindingMismatch);
        }
        let capsule_digest = self
            .audit_capsule
            .digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if capsule_digest != self.statement.audit_capsule_digest {
            return Err(PrivateSettlementValidationError::AuditCapsuleDigestMismatch);
        }
        self.delta.validate_against(&self.statement)?;
        if self.delta.proof_digest != self.proof_digest() {
            return Err(PrivateSettlementValidationError::ProofDigestMismatch);
        }
        let delta_digest = self
            .delta
            .digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        let payload_digest = self
            .sidecar_material_digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        let canonical_payload_bytes = self
            .sidecar_material_bytes_len()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if leg.delta_digest != delta_digest || leg.payload_digest != payload_digest {
            return Err(PrivateSettlementValidationError::ManifestPayloadMismatch);
        }
        Ok((delta_digest, payload_digest, canonical_payload_bytes))
    }
}

/// Immutable restricted leg material persisted before availability shares are issued.
///
/// The manifest must carry reserved-zero availability certificate digests for
/// every leg. Proof bytes, the opaque delta, and the encrypted audit capsule
/// are already final and content addressed by `availability_body.payload_digest`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementProvisionalLegMaterialV1 {
    /// Provisional material wire version.
    pub version: u8,
    /// Exact all-leg manifest with reserved-zero availability digests.
    pub manifest: AtomicPrivateSettlementV1,
    /// Governed local audit policy.
    pub audit_policy: PrivateSettlementAuditPolicyV1,
    /// Exact four-validator committee authority.
    pub committee_authority: PrivateSettlementCommitteeAuthorityV1,
    /// Restricted proof statement.
    pub statement: PrivateSettlementProofStatementV1,
    /// Native proof bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub proof: Vec<u8>,
    /// Fixed-shape opaque state delta.
    pub delta: PrivateSettlementDeltaV1,
    /// Padded auditor-only encrypted capsule.
    pub audit_capsule: PrivateSettlementAuditCapsuleV1,
    /// Exact body each committee member must sign after durable persistence.
    pub availability_body: PrivateSettlementSidecarAvailabilityBodyV1,
}

impl fmt::Debug for PrivateSettlementProvisionalLegMaterialV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateSettlementProvisionalLegMaterialV1")
            .field("bundle_id", &self.manifest.bundle_id)
            .field("leg_ordinal", &self.statement.leg_ordinal)
            .field("route", &self.statement.route)
            .field("payload_digest", &self.availability_body.payload_digest)
            .finish_non_exhaustive()
    }
}

impl PrivateSettlementProvisionalLegMaterialV1 {
    /// Build the exact payload that will carry a completed certificate.
    #[must_use]
    pub fn payload_with_certificate(
        &self,
        availability: PrivateSettlementSidecarAvailabilityV1,
    ) -> PrivateSettlementLegPayloadV1 {
        PrivateSettlementLegPayloadV1 {
            statement: self.statement.clone(),
            proof: self.proof.clone(),
            delta: self.delta.clone(),
            audit_capsule: self.audit_capsule.clone(),
            availability,
        }
    }

    /// Compute the content address of the exact immutable restricted material.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if material cannot be canonically encoded.
    pub fn payload_digest(&self) -> Result<Hash, norito::Error> {
        self.payload_with_certificate(PrivateSettlementSidecarAvailabilityV1 {
            body: self.availability_body,
            signers_bitmap: 0,
            aggregate_signature: Vec::new(),
        })
        .payload_digest()
    }

    /// Return the canonical byte length of material excluding the certificate.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if material cannot be canonically encoded.
    pub fn sidecar_material_bytes_len(&self) -> Result<usize, norito::Error> {
        self.payload_with_certificate(PrivateSettlementSidecarAvailabilityV1 {
            body: self.availability_body,
            signers_bitmap: 0,
            aggregate_signature: Vec::new(),
        })
        .sidecar_material_bytes_len()
    }

    /// Validate every provisional manifest, material, authority, and body binding.
    ///
    /// Cryptographic committee proofs of possession and shares are verified by
    /// the node runtime after this structural boundary.
    ///
    /// # Errors
    ///
    /// Returns a typed fail-closed validation error.
    pub fn validate(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        self.committee_authority.validate()?;
        let payload = self.payload_with_certificate(PrivateSettlementSidecarAvailabilityV1 {
            body: self.availability_body,
            signers_bitmap: 0,
            aggregate_signature: Vec::new(),
        });
        let (_, payload_digest, payload_bytes) =
            payload.validate_material_against_v1(&self.manifest, &self.audit_policy, true)?;
        self.availability_body.validate_shape()?;
        let authority_digest = self
            .committee_authority
            .digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if self.committee_authority.route != self.statement.route
            || self.availability_body.network_id != self.manifest.network_id
            || self.availability_body.bundle_id != self.manifest.bundle_id
            || self.availability_body.leg_ordinal != self.statement.leg_ordinal
            || self.availability_body.route != self.statement.route
            || self.availability_body.authority_digest != authority_digest
            || self.audit_capsule.aad.authority_digest != authority_digest
            || self.audit_capsule.aad.authority_context_height
                != self.availability_body.authority_context_height
            || self.availability_body.authority_context_height
                != self.manifest.authority_context_height
            || self.availability_body.payload_digest != payload_digest
            || usize::try_from(self.availability_body.payload_bytes).ok() != Some(payload_bytes)
            || self.availability_body.retention_until_height < self.manifest.expiry_height
        {
            return Err(PrivateSettlementValidationError::SidecarAvailabilityMismatch);
        }
        Ok(())
    }
}

/// Compute the canonical domain-separated digest of settlement proof bytes.
///
/// This helper lets restricted clients verify committee responses without
/// reconstructing an encrypted capsule or a complete leg payload.
#[must_use]
pub fn private_settlement_proof_digest_v1(proof: &[u8]) -> Hash {
    Hash::new_from_chunks(&[
        PROOF_DIGEST_DOMAIN_V1,
        &u64::try_from(proof.len())
            .expect("proof length fits u64")
            .to_le_bytes(),
        proof,
    ])
}

/// Exact purpose-separated body signed by a local auditor.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditApprovalBodyV1 {
    /// Approval wire version.
    pub version: u8,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Stable bundle identifier.
    pub bundle_id: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Participant dataspace.
    pub dataspace_id: DataSpaceId,
    /// Auditor producing this approval.
    pub auditor_id: AccountId,
    /// Governed policy digest.
    pub audit_policy_digest: Hash,
    /// Exact policy key epoch.
    pub audit_key_epoch: u64,
    /// Restricted proof digest.
    pub proof_digest: Hash,
    /// Encrypted capsule digest.
    pub capsule_digest: Hash,
    /// Fixed-shape delta digest.
    pub delta_digest: Hash,
    /// Current private root.
    pub old_root: PrivacyRootV1,
    /// Successor private root.
    pub new_root: PrivacyRootV1,
    /// Last global height at which this approval is valid.
    pub expiry_height: u64,
}

impl fmt::Debug for PrivateSettlementAuditApprovalBodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditApprovalBodyV1(<restricted>)")
    }
}

/// Signed local-auditor approval required before participant Prepare.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuditApprovalV1 {
    /// Exact purpose-separated signed body.
    pub body: PrivateSettlementAuditApprovalBodyV1,
    /// Auditor signature over `body`.
    pub signature: SignatureOf<PrivateSettlementAuditApprovalBodyV1>,
}

impl fmt::Debug for PrivateSettlementAuditApprovalV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateSettlementAuditApprovalV1(<restricted>)")
    }
}

impl PrivateSettlementAuditApprovalV1 {
    /// Compute the purpose-separated digest of the complete signed approval.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the approval cannot be canonically encoded.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(AUDIT_APPROVAL_DIGEST_DOMAIN_V1, self)
    }

    /// Verify membership, signature, policy/key epoch, and height validity.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an invalid or unauthorized approval.
    pub fn verify(
        &self,
        policy: &PrivateSettlementAuditPolicyV1,
        at_height: u64,
    ) -> Result<(), PrivateSettlementValidationError> {
        policy.validate()?;
        if self.body.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.body.version,
            });
        }
        if !policy.is_active_at(at_height)
            || self.body.expiry_height < at_height
            || self.body.dataspace_id != policy.body.dataspace_id
            || self.body.audit_policy_digest != policy.policy_digest
            || self.body.audit_key_epoch != policy.body.key_epoch
        {
            return Err(PrivateSettlementValidationError::StaleAuditApproval);
        }
        let auditor = policy
            .body
            .auditors
            .iter()
            .find(|auditor| auditor.auditor_id == self.body.auditor_id)
            .ok_or(PrivateSettlementValidationError::UnauthorizedAuditor)?;
        self.signature
            .verify(&auditor.signing_key, &self.body)
            .map_err(|_| PrivateSettlementValidationError::InvalidAuditSignature)
    }
}

/// Validate one local-auditor approval against the exact encrypted leg.
///
/// This checks every purpose-separated policy, proof, capsule, delta, root,
/// route, bundle, network, epoch, and expiry binding without imposing the
/// policy threshold.  Durable collectors use it while gathering a multi-
/// auditor threshold one approval at a time.
///
/// # Errors
///
/// Returns a typed error if the signature, governed membership, lifecycle, or
/// any immutable leg binding is invalid.
pub fn validate_private_settlement_audit_approval_v1(
    approval: &PrivateSettlementAuditApprovalV1,
    policy: &PrivateSettlementAuditPolicyV1,
    payload: &PrivateSettlementLegPayloadV1,
    at_height: u64,
) -> Result<(), PrivateSettlementValidationError> {
    approval.verify(policy, at_height)?;
    let delta_digest = payload
        .delta
        .digest()
        .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
    if approval.body.network_id != payload.statement.network_id
        || approval.body.bundle_id != payload.statement.bundle_id
        || approval.body.leg_ordinal != payload.statement.leg_ordinal
        || approval.body.dataspace_id != payload.statement.route.dataspace_id
        || approval.body.proof_digest != payload.delta.proof_digest
        || approval.body.capsule_digest != payload.delta.capsule_digest
        || approval.body.delta_digest != delta_digest
        || approval.body.old_root != payload.delta.old_root
        || approval.body.new_root != payload.delta.new_root
        || approval.body.expiry_height != payload.statement.expiry_height
    {
        return Err(PrivateSettlementValidationError::AuditApprovalBindingMismatch);
    }
    Ok(())
}

/// Validate a canonical set of distinct local-auditor approvals.
///
/// # Errors
///
/// Returns a typed error if signatures are invalid, bindings differ, approvals
/// are duplicated or unordered, or the governed threshold is not met.
pub fn validate_private_settlement_audit_approvals_v1(
    approvals: &[PrivateSettlementAuditApprovalV1],
    policy: &PrivateSettlementAuditPolicyV1,
    payload: &PrivateSettlementLegPayloadV1,
    at_height: u64,
) -> Result<(), PrivateSettlementValidationError> {
    policy.validate()?;
    if approvals.len() < usize::from(policy.body.min_approvals) {
        return Err(
            PrivateSettlementValidationError::InsufficientAuditApprovals {
                actual: approvals.len(),
                required: policy.body.min_approvals,
            },
        );
    }
    let mut previous = None;
    for approval in approvals {
        validate_private_settlement_audit_approval_v1(approval, policy, payload, at_height)?;
        if previous
            .as_ref()
            .is_some_and(|auditor| auditor >= &approval.body.auditor_id)
        {
            return Err(PrivateSettlementValidationError::NonCanonicalApprovalOrder);
        }
        previous = Some(approval.body.auditor_id.clone());
    }
    Ok(())
}

/// Native AMX phase certified for a private settlement leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "phase",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum PrivateSettlementPhaseV1 {
    /// Proof, audit, state, and sidecar availability have been durably staged.
    Prepare,
    /// The committee binds its leg to the exact whole-bundle manifest.
    Commit,
}

/// Route-free four-validator committee roster stored once in an authority catalog.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementCommitteeRosterV1 {
    /// Canonical hash of `validators`.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Exactly four ordered validator identities.
    pub validators: Vec<PeerId>,
    /// BLS proofs of possession aligned one-for-one with `validators`.
    pub validator_pops: Vec<Vec<u8>>,
}

impl PrivateSettlementCommitteeRosterV1 {
    /// Validate exact four-validator committee shape and proofs of possession.
    ///
    /// # Errors
    ///
    /// Returns a typed structural error.
    pub fn validate(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.validators.len() != PRIVATE_SETTLEMENT_COMMITTEE_VALIDATORS_V1
            || self.validator_pops.len() != self.validators.len()
            || self.validators.iter().collect::<BTreeSet<_>>().len() != self.validators.len()
            || self
                .validator_pops
                .iter()
                .any(|pop| pop.len() != PRIVATE_SETTLEMENT_BLS_BYTES_V1)
            || self.validator_set_hash != HashOf::new(&self.validators)
        {
            return Err(PrivateSettlementValidationError::InvalidCommitteeAuthority);
        }
        Ok(())
    }

    /// Reconstruct the route-bound authority committed by a phase body.
    #[must_use]
    pub fn with_route(
        &self,
        route: PrivateSettlementRouteV1,
    ) -> PrivateSettlementCommitteeAuthorityV1 {
        PrivateSettlementCommitteeAuthorityV1 {
            route,
            validator_set_hash: self.validator_set_hash,
            validators: self.validators.clone(),
            validator_pops: self.validator_pops.clone(),
        }
    }
}

/// Route-bound committee authority used by sidecars and phase signatures.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementCommitteeAuthorityV1 {
    /// Exact route governed by this committee.
    pub route: PrivateSettlementRouteV1,
    /// Canonical hash of `validators`.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Exactly four ordered validator identities.
    pub validators: Vec<PeerId>,
    /// BLS proofs of possession aligned one-for-one with `validators`.
    pub validator_pops: Vec<Vec<u8>>,
}

impl PrivateSettlementCommitteeAuthorityV1 {
    /// Return the route-free roster material stored in compact public catalogs.
    #[must_use]
    pub fn roster(&self) -> PrivateSettlementCommitteeRosterV1 {
        PrivateSettlementCommitteeRosterV1 {
            validator_set_hash: self.validator_set_hash,
            validators: self.validators.clone(),
            validator_pops: self.validator_pops.clone(),
        }
    }

    /// Compute the compact authority-record digest signed into phase bodies.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the record cannot be encoded.
    pub fn digest(&self) -> Result<Hash, norito::Error> {
        canonical_hash(AUTHORITY_DIGEST_DOMAIN_V1, self)
    }

    /// Validate exact four-validator committee shape and proofs of possession.
    ///
    /// # Errors
    ///
    /// Returns a typed structural error.
    pub fn validate(&self) -> Result<(), PrivateSettlementValidationError> {
        if hash_is_zero(&self.route.lane_incarnation) || self.roster().validate().is_err() {
            return Err(PrivateSettlementValidationError::InvalidCommitteeAuthority);
        }
        Ok(())
    }
}

/// Compact two-level committee catalog shared by every public bundle object.
///
/// Phase certificates keep indexing the logical leg slot. The corresponding
/// entry in `leg_roster_indices` selects a route-free roster, which is combined
/// with the manifest leg route before authority-digest or QC verification.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAuthorityCatalogV1 {
    /// Unique committee rosters in canonical first-use order.
    pub rosters: Vec<PrivateSettlementCommitteeRosterV1>,
    /// Roster index for every canonical manifest leg.
    pub leg_roster_indices: Vec<u8>,
}

impl PrivateSettlementAuthorityCatalogV1 {
    /// Build the canonical compact catalog from route-bound per-leg authorities.
    ///
    /// # Errors
    ///
    /// Returns a typed error if a route is missing, a roster is malformed, or
    /// the same validator-set hash is supplied with different roster material.
    pub fn from_leg_authorities(
        manifest: &AtomicPrivateSettlementV1,
        authorities: &[PrivateSettlementCommitteeAuthorityV1],
    ) -> Result<Self, PrivateSettlementValidationError> {
        manifest.validate()?;
        if authorities.len() != manifest.legs.len() {
            return Err(PrivateSettlementValidationError::InvalidAuthorityCatalog);
        }
        let mut catalog = Self::default();
        for (leg, authority) in manifest.legs.iter().zip(authorities) {
            authority.validate()?;
            if authority.route != leg.route {
                return Err(PrivateSettlementValidationError::InvalidAuthorityCatalog);
            }
            let roster = authority.roster();
            let roster_index = if let Some(index) = catalog
                .rosters
                .iter()
                .position(|candidate| candidate.validator_set_hash == roster.validator_set_hash)
            {
                if catalog.rosters[index] != roster {
                    return Err(PrivateSettlementValidationError::InvalidAuthorityCatalog);
                }
                index
            } else {
                let index = catalog.rosters.len();
                catalog.rosters.push(roster);
                index
            };
            catalog.leg_roster_indices.push(
                u8::try_from(roster_index)
                    .map_err(|_| PrivateSettlementValidationError::InvalidAuthorityCatalog)?,
            );
        }
        catalog.validate_for_manifest(manifest)?;
        Ok(catalog)
    }

    /// Validate bounds, uniqueness, references, and canonical first-use order.
    ///
    /// # Errors
    ///
    /// Returns a typed error for any malformed or non-canonical catalog.
    pub fn validate_for_manifest(
        &self,
        manifest: &AtomicPrivateSettlementV1,
    ) -> Result<(), PrivateSettlementValidationError> {
        manifest.validate()?;
        if self.leg_roster_indices.len() != manifest.legs.len()
            || self.rosters.is_empty()
            || self.rosters.len() > manifest.legs.len()
            || self.rosters.len() > usize::from(u8::MAX)
        {
            return Err(PrivateSettlementValidationError::InvalidAuthorityCatalog);
        }
        let mut roster_hashes = BTreeSet::new();
        for roster in &self.rosters {
            roster.validate()?;
            if !roster_hashes.insert(roster.validator_set_hash) {
                return Err(PrivateSettlementValidationError::InvalidAuthorityCatalog);
            }
        }
        let mut seen = vec![false; self.rosters.len()];
        let mut next_first_use = 0_usize;
        for &roster_index in &self.leg_roster_indices {
            let roster_index = usize::from(roster_index);
            let Some(was_seen) = seen.get_mut(roster_index) else {
                return Err(PrivateSettlementValidationError::InvalidAuthorityCatalog);
            };
            if !*was_seen {
                if roster_index != next_first_use {
                    return Err(PrivateSettlementValidationError::InvalidAuthorityCatalog);
                }
                *was_seen = true;
                next_first_use += 1;
            }
        }
        if next_first_use != self.rosters.len() {
            return Err(PrivateSettlementValidationError::InvalidAuthorityCatalog);
        }
        Ok(())
    }

    /// Resolve one logical leg slot to its exact route-bound authority.
    ///
    /// # Errors
    ///
    /// Returns a typed error if the manifest ordinal or catalog reference is invalid.
    pub fn authority_for_leg(
        &self,
        manifest: &AtomicPrivateSettlementV1,
        leg_index: usize,
    ) -> Result<PrivateSettlementCommitteeAuthorityV1, PrivateSettlementValidationError> {
        let leg = manifest
            .legs
            .get(leg_index)
            .ok_or(PrivateSettlementValidationError::InvalidAuthorityCatalog)?;
        if usize::from(leg.ordinal) != leg_index {
            return Err(PrivateSettlementValidationError::InvalidAuthorityCatalog);
        }
        let roster_index = self
            .leg_roster_indices
            .get(leg_index)
            .copied()
            .map(usize::from)
            .ok_or(PrivateSettlementValidationError::InvalidAuthorityCatalog)?;
        self.rosters
            .get(roster_index)
            .map(|roster| roster.with_route(leg.route))
            .ok_or(PrivateSettlementValidationError::InvalidAuthorityCatalog)
    }
}

/// Context bound by a participant committee phase certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementPhaseBodyV1 {
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Stable settlement bundle identifier.
    pub bundle_id: Hash,
    /// Digest of the exact whole manifest.
    pub manifest_digest: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Exact participant route.
    pub route: PrivateSettlementRouteV1,
    /// Digest of the fixed-shape state delta.
    pub delta_digest: Hash,
    /// Digest of the reconstructed route-bound authority record.
    pub authority_digest: Hash,
    /// Digest of the exact all-leg Prepare barrier.
    ///
    /// Prepare bodies reserve the all-zero value because the barrier is not yet
    /// complete. Commit bodies must carry the same non-zero digest over the
    /// manifest, authority catalog, every delta, and every Prepare QC.
    pub prepared_bundle_digest: Hash,
    /// Certified protocol phase.
    pub phase: PrivateSettlementPhaseV1,
    /// Global/catalog authority context height.
    pub authority_context_height: u64,
    /// Final admissible global height.
    pub expiry_height: u64,
}

impl PrivateSettlementPhaseBodyV1 {
    /// Canonical purpose-separated bytes signed by one participant validator.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the body cannot be canonically encoded.
    pub fn signature_preimage(&self) -> Result<Vec<u8>, norito::Error> {
        let body = norito::encode_canonical(self)?;
        let body_len = u64::try_from(body.len())
            .map_err(|_| norito::Error::Io(std::io::Error::other("phase body is too large")))?;
        let mut preimage = Vec::with_capacity(
            PHASE_SIGNATURE_DOMAIN_V1.len() + std::mem::size_of::<u64>() + body.len(),
        );
        preimage.extend_from_slice(PHASE_SIGNATURE_DOMAIN_V1);
        preimage.extend_from_slice(&body_len.to_le_bytes());
        preimage.extend_from_slice(&body);
        Ok(preimage)
    }
}

/// One independently authenticated participant phase vote before aggregation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementPhaseVoteV1 {
    /// Vote wire version.
    pub version: u8,
    /// Exact purpose-separated phase body.
    pub body: PrivateSettlementPhaseBodyV1,
    /// Exact committee validator producing the vote.
    pub signer: PeerId,
    /// Compressed BLS-normal signature over the canonical phase body.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub signature: Vec<u8>,
}

impl PrivateSettlementPhaseVoteV1 {
    /// Validate fixed vote wire shape before roster and signature verification.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a malformed or unsupported vote.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if self.signature.len() != PRIVATE_SETTLEMENT_BLS_BYTES_V1
            || hash_is_zero(&self.body.bundle_id)
            || hash_is_zero(&self.body.manifest_digest)
            || hash_is_zero(&self.body.delta_digest)
            || hash_is_zero(&self.body.authority_digest)
            || match self.body.phase {
                PrivateSettlementPhaseV1::Prepare => {
                    !hash_is_zero(&self.body.prepared_bundle_digest)
                }
                PrivateSettlementPhaseV1::Commit => hash_is_zero(&self.body.prepared_bundle_digest),
            }
        {
            return Err(PrivateSettlementValidationError::InvalidPhaseVote);
        }
        Ok(())
    }
}

/// Compact phase certificate referencing a receipt-level logical leg slot.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementPhaseCertificateV1 {
    /// Exact signed phase body.
    pub body: PrivateSettlementPhaseBodyV1,
    /// Logical leg-slot index resolved through the catalog's `leg_roster_indices` map.
    pub authority_catalog_index: u8,
    /// Four-bit LSB-first signer bitmap.
    pub signers_bitmap: u8,
    /// Compressed aggregate BLS-normal signature.
    pub aggregate_signature: Vec<u8>,
}

impl PrivateSettlementPhaseCertificateV1 {
    /// Validate bitmap and signature wire shape.
    ///
    /// Cryptographic aggregate verification is performed by core against the
    /// authority record after this inexpensive structural check.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed certificate shape.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.signers_bitmap & !0x0f != 0
            || self.signers_bitmap.count_ones() != u32::from(PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1)
            || self.aggregate_signature.len() != PRIVATE_SETTLEMENT_BLS_BYTES_V1
            || hash_is_zero(&self.body.bundle_id)
            || hash_is_zero(&self.body.manifest_digest)
            || hash_is_zero(&self.body.delta_digest)
            || hash_is_zero(&self.body.authority_digest)
            || match self.body.phase {
                PrivateSettlementPhaseV1::Prepare => {
                    !hash_is_zero(&self.body.prepared_bundle_digest)
                }
                PrivateSettlementPhaseV1::Commit => hash_is_zero(&self.body.prepared_bundle_digest),
            }
        {
            return Err(PrivateSettlementValidationError::InvalidPhaseCertificate);
        }
        Ok(())
    }
}

/// Complete all-Prepare barrier that every Commit vote must bind.
///
/// The vectors are aligned by canonical leg ordinal. Core independently
/// verifies every authority, delta, and Prepare QC before recomputing
/// `prepared_bundle_digest`; carrying this material prevents a coordinator
/// from substituting one leg or certified statement for another at Commit
/// time. The digest normalizes quorum-equivalent certificate encodings: two
/// exact three-of-four signer subsets over the same signed body certify the
/// same logical barrier and therefore cannot fork coordinator recovery.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementPrepareBarrierV1 {
    /// Barrier wire version.
    pub version: u8,
    /// Exact finalized public manifest.
    pub manifest: AtomicPrivateSettlementV1,
    /// Compact participant authority catalog with one logical slot per leg.
    pub authority_catalog: PrivateSettlementAuthorityCatalogV1,
    /// Every fixed-shape delta in canonical leg order.
    pub deltas: Vec<PrivateSettlementDeltaV1>,
    /// Every cryptographically valid Prepare QC in canonical leg order.
    pub prepare_certificates: Vec<PrivateSettlementPhaseCertificateV1>,
    /// Canonical digest of all preceding fields except `version`.
    pub prepared_bundle_digest: Hash,
}

impl PrivateSettlementPrepareBarrierV1 {
    /// Recompute the canonical complete-bundle digest.
    ///
    /// Aggregate signatures and signer bitmaps are deliberately excluded from
    /// the digest. They are independently verified against the authority
    /// catalog, while the signed body and authority-catalog index are included.
    /// This makes every valid exact three-of-four certificate for one statement
    /// quorum-equivalent and preserves restart liveness with one unavailable
    /// validator.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the complete barrier material cannot be encoded.
    pub fn computed_prepared_bundle_digest(&self) -> Result<Hash, norito::Error> {
        let certified_statements = self
            .prepare_certificates
            .iter()
            .map(|certificate| (certificate.authority_catalog_index, certificate.body))
            .collect::<Vec<_>>();
        let material = (
            self.manifest.clone(),
            self.authority_catalog.clone(),
            self.deltas.clone(),
            certified_statements,
        );
        canonical_hash(PREPARED_BUNDLE_DIGEST_DOMAIN_V1, &material)
    }

    /// Return whether two barriers carry the same certified statements.
    ///
    /// Signer bitmaps and aggregate signatures may differ because any valid
    /// three-of-four subset certifies the same Prepare body. This comparison
    /// deliberately remains structural: callers must independently validate
    /// both barriers and their aggregate signatures before treating them as
    /// quorum-equivalent.
    #[must_use]
    pub fn quorum_equivalent_to(&self, other: &Self) -> bool {
        self.version == other.version
            && self.manifest == other.manifest
            && self.authority_catalog == other.authority_catalog
            && self.deltas == other.deltas
            && self.prepared_bundle_digest == other.prepared_bundle_digest
            && self.prepare_certificates.len() == other.prepare_certificates.len()
            && self
                .prepare_certificates
                .iter()
                .zip(&other.prepare_certificates)
                .all(|(left, right)| {
                    left.body == right.body
                        && left.authority_catalog_index == right.authority_catalog_index
                })
    }

    /// Validate bounded canonical vector alignment before cryptographic checks.
    ///
    /// # Errors
    ///
    /// Returns a typed error for unsupported, incomplete, or misaligned material.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        self.manifest.validate()?;
        let leg_count = self.manifest.legs.len();
        if self.deltas.len() != leg_count
            || self.prepare_certificates.len() != leg_count
            || hash_is_zero(&self.prepared_bundle_digest)
        {
            return Err(PrivateSettlementValidationError::InvalidPrepareBarrier);
        }
        self.authority_catalog
            .validate_for_manifest(&self.manifest)
            .map_err(|_| PrivateSettlementValidationError::InvalidPrepareBarrier)?;
        let mut output_recipients = BTreeSet::new();
        for (index, ((manifest_leg, delta), certificate)) in self
            .manifest
            .legs
            .iter()
            .zip(&self.deltas)
            .zip(&self.prepare_certificates)
            .enumerate()
        {
            let ordinal =
                u8::try_from(index).expect("private settlement has at most 255 participant legs");
            if manifest_leg.ordinal != ordinal
                || delta.leg_ordinal != ordinal
                || delta.route != manifest_leg.route
                || delta.validate_public_shape().is_err()
                || delta
                    .encrypted_outputs
                    .iter()
                    .any(|output| !output_recipients.insert(output.recipient))
                || certificate.authority_catalog_index != ordinal
                || certificate.body.phase != PrivateSettlementPhaseV1::Prepare
                || certificate.body.leg_ordinal != ordinal
                || certificate.body.route != manifest_leg.route
                || certificate.validate_shape().is_err()
            {
                return Err(PrivateSettlementValidationError::InvalidPrepareBarrier);
            }
        }
        Ok(())
    }
}

/// Finalized receipt row for one private settlement leg.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementLegReceiptV1 {
    /// Fixed-shape public state delta.
    pub delta: PrivateSettlementDeltaV1,
    /// Participant Prepare certificate.
    pub prepare: PrivateSettlementPhaseCertificateV1,
    /// Participant Commit certificate.
    pub commit: PrivateSettlementPhaseCertificateV1,
}

/// Complete committee-certified bundle carried before global block finality.
///
/// The actual finalization height is deliberately absent: a sponsor cannot
/// predict the block that will include its signed transaction. Consensus adds
/// that height when it constructs the terminal [`PrivateSettlementReceiptV1`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementCommitBundleV1 {
    /// Carrier wire version.
    pub version: u8,
    /// Exact public manifest.
    pub manifest: AtomicPrivateSettlementV1,
    /// Compact participant authority catalog with one logical slot per leg.
    pub authority_catalog: PrivateSettlementAuthorityCatalogV1,
    /// One Prepare/Commit-certified row per canonical participant leg.
    pub legs: Vec<PrivateSettlementLegReceiptV1>,
}

impl PrivateSettlementCommitBundleV1 {
    /// Return the canonical direct-instruction byte length used for carrier preflight.
    ///
    /// This includes the registered [`crate::isi::InstructionBox`] framing for
    /// [`crate::isi::private_settlement::FinalizeAtomicPrivateSettlementV1`].
    /// Runtime admission additionally limits the complete sponsor-signed
    /// transaction, including its authority, metadata, fee intent, and signature.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the complete boxed instruction cannot be encoded.
    pub fn canonical_carrier_bytes_len(&self) -> Result<usize, norito::Error> {
        let instruction =
            crate::isi::private_settlement::FinalizeAtomicPrivateSettlementV1::new(self.clone());
        let boxed = crate::isi::InstructionBox::from(instruction);
        norito::encode_canonical(&boxed).map(|encoded| encoded.len())
    }

    /// Construct the terminal receipt at the deterministic inclusion height.
    #[must_use]
    pub fn into_receipt(self, finalized_height: u64) -> PrivateSettlementReceiptV1 {
        PrivateSettlementReceiptV1 {
            version: self.version,
            manifest: self.manifest,
            authority_catalog: self.authority_catalog,
            legs: self.legs,
            finalized_height,
        }
    }
}

/// Compact globally finalized private settlement receipt.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementReceiptV1 {
    /// Receipt wire version.
    pub version: u8,
    /// Exact public manifest.
    pub manifest: AtomicPrivateSettlementV1,
    /// Compact participant authority catalog with one logical slot per leg.
    pub authority_catalog: PrivateSettlementAuthorityCatalogV1,
    /// One finalized leg record per canonical participant leg.
    pub legs: Vec<PrivateSettlementLegReceiptV1>,
    /// Global height at which every delta became active atomically.
    pub finalized_height: u64,
}

impl PrivateSettlementReceiptV1 {
    /// Return the canonical pre-finality direct-instruction length represented by this receipt.
    ///
    /// The consensus-assigned `finalized_height` is not part of the carrier.
    /// This reconstructs the registered finalization instruction for deterministic
    /// WSV preflight; signed-transaction admission separately measures the exact
    /// complete sponsor-signed transaction.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the represented commit bundle cannot be encoded.
    pub fn canonical_carrier_bytes_len(&self) -> Result<usize, norito::Error> {
        PrivateSettlementCommitBundleV1 {
            version: self.version,
            manifest: self.manifest.clone(),
            authority_catalog: self.authority_catalog.clone(),
            legs: self.legs.clone(),
        }
        .canonical_carrier_bytes_len()
    }

    /// Validate the compact receipt's complete non-cryptographic shape and bindings.
    ///
    /// # Errors
    ///
    /// Returns a typed error before aggregate signature verification or state application.
    pub fn validate_shape(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        self.manifest.validate()?;
        if self.finalized_height < self.manifest.authority_context_height
            || self.finalized_height > self.manifest.expiry_height
            || self.legs.len() != self.manifest.legs.len()
        {
            return Err(PrivateSettlementValidationError::InvalidReceiptShape);
        }
        self.authority_catalog
            .validate_for_manifest(&self.manifest)
            .map_err(|_| PrivateSettlementValidationError::InvalidReceiptShape)?;
        let manifest_digest = self
            .manifest
            .manifest_digest()
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        let mut prepared_bundle_digest = None;
        let mut output_recipients = BTreeSet::new();
        for (index, (manifest_leg, leg)) in self.manifest.legs.iter().zip(&self.legs).enumerate() {
            let authority = self
                .authority_catalog
                .authority_for_leg(&self.manifest, index)
                .map_err(|_| PrivateSettlementValidationError::InvalidReceiptShape)?;
            authority.validate()?;
            leg.delta.validate_public_shape()?;
            if leg
                .delta
                .encrypted_outputs
                .iter()
                .any(|output| !output_recipients.insert(output.recipient))
            {
                return Err(PrivateSettlementValidationError::DuplicateStateItem);
            }
            leg.prepare.validate_shape()?;
            leg.commit.validate_shape()?;
            let ordinal = u8::try_from(index).expect("receipt has at most 255 legs");
            let authority_digest = authority
                .digest()
                .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
            let delta_digest = leg
                .delta
                .digest()
                .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
            if authority.route != manifest_leg.route
                || leg.delta.bundle_id != self.manifest.bundle_id
                || leg.delta.leg_ordinal != ordinal
                || leg.delta.route != manifest_leg.route
                || leg.delta.pool_id != manifest_leg.pool_id
                || leg.delta.asset_binding_commitment != manifest_leg.asset_binding_commitment
                || leg.delta.audit_policy_digest != manifest_leg.audit_policy_digest
                || delta_digest != manifest_leg.delta_digest
                || leg.prepare.authority_catalog_index != ordinal
                || leg.commit.authority_catalog_index != ordinal
                || leg.prepare.body.phase != PrivateSettlementPhaseV1::Prepare
                || leg.commit.body.phase != PrivateSettlementPhaseV1::Commit
            {
                return Err(PrivateSettlementValidationError::ReceiptBindingMismatch);
            }
            if prepared_bundle_digest
                .replace(leg.commit.body.prepared_bundle_digest)
                .is_some_and(|digest| digest != leg.commit.body.prepared_bundle_digest)
            {
                return Err(PrivateSettlementValidationError::ReceiptBindingMismatch);
            }
            for body in [&leg.prepare.body, &leg.commit.body] {
                if body.network_id != self.manifest.network_id
                    || body.bundle_id != self.manifest.bundle_id
                    || body.manifest_digest != manifest_digest
                    || body.leg_ordinal != ordinal
                    || body.route != manifest_leg.route
                    || body.delta_digest != delta_digest
                    || body.authority_digest != authority_digest
                    || body.authority_context_height != self.manifest.authority_context_height
                    || body.expiry_height != self.manifest.expiry_height
                {
                    return Err(PrivateSettlementValidationError::ReceiptBindingMismatch);
                }
            }
        }
        let encoded = norito::encode_canonical(self)
            .map_err(|_| PrivateSettlementValidationError::CanonicalEncoding)?;
        if encoded.len() > PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 {
            return Err(PrivateSettlementValidationError::ReceiptTooLarge {
                bytes: encoded.len(),
            });
        }
        Ok(())
    }
}

/// Public terminal reason class for an aborted private settlement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "reason",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum PrivateSettlementAbortReasonV1 {
    /// Bundle reached its globally defined expiry height.
    Expired,
    /// At least one participant committee rejected its opaque leg.
    ParticipantRejected,
    /// The required local auditor threshold was unavailable or rejected the leg.
    AuditUnavailable,
    /// Restricted proof/capsule availability could not be certified.
    SidecarUnavailable,
}

/// Optional public replay marker for an aborted private settlement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivateSettlementAbortReceiptV1 {
    /// Receipt wire version.
    pub version: u8,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Stable settlement bundle identifier.
    pub bundle_id: Hash,
    /// Digest of the exact public manifest.
    pub manifest_digest: Hash,
    /// Final global height of the abort marker.
    pub finalized_height: u64,
    /// Public non-sensitive reason class.
    pub reason: PrivateSettlementAbortReasonV1,
}

impl PrivateSettlementAbortReceiptV1 {
    /// Validate version and non-zero public replay fields.
    ///
    /// # Errors
    ///
    /// Returns a typed structural error.
    pub fn validate(&self) -> Result<(), PrivateSettlementValidationError> {
        if self.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1 {
            return Err(PrivateSettlementValidationError::UnsupportedVersion {
                actual: self.version,
            });
        }
        if hash_is_zero(&self.bundle_id)
            || hash_is_zero(&self.manifest_digest)
            || self.finalized_height == 0
        {
            return Err(PrivateSettlementValidationError::InvalidAbortReceipt);
        }
        Ok(())
    }
}

/// Structural validation failures for atomic private settlement wire objects.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PrivateSettlementValidationError {
    /// An unsupported wire version was supplied.
    #[error("unsupported atomic private settlement version {actual}")]
    UnsupportedVersion {
        /// Actual version.
        actual: u8,
    },
    /// Participant count is outside 2 through 255.
    #[error("atomic private settlement participant count {count} is outside 2..=255")]
    ParticipantCount {
        /// Actual participant count.
        count: usize,
    },
    /// Authority and expiry heights are invalid.
    #[error("atomic private settlement expiry must be after a non-zero authority context")]
    InvalidExpiry,
    /// Reimbursement leg ordinal is outside the participant set.
    #[error("atomic private settlement reimbursement leg is out of range")]
    InvalidReimbursementLeg,
    /// A leg ordinal differs from its canonical vector index.
    #[error("leg at index {index} has non-canonical ordinal {actual}")]
    NonCanonicalOrdinal {
        /// Vector index.
        index: usize,
        /// Supplied ordinal.
        actual: u8,
    },
    /// Routes are duplicated or not strictly ordered.
    #[error("private settlement routes must be strictly ordered and unique")]
    NonCanonicalRouteOrder,
    /// More than one leg routes to the same participant dataspace.
    #[error("private settlement permits exactly one leg per participant dataspace")]
    DuplicateDataspace,
    /// A cryptographic commitment or identifier used the reserved zero value.
    #[error("private settlement contains a reserved zero commitment")]
    ZeroCommitment,
    /// Stable bundle identifier does not match public intent material.
    #[error("private settlement bundle id does not match public intent material")]
    BundleIdMismatch,
    /// Exact public fee intent does not match its committed digest.
    #[error("private settlement public fee intent digest mismatch")]
    FeeIntentDigestMismatch,
    /// Exact public fee intent is structurally invalid.
    #[error("private settlement public fee intent is invalid")]
    InvalidFeeIntent,
    /// Canonical encoding unexpectedly failed.
    #[error("private settlement canonical encoding failed")]
    CanonicalEncoding,
    /// A root or key epoch is zero, non-contiguous, or otherwise invalid.
    #[error("private settlement epoch transition is invalid")]
    InvalidEpoch,
    /// Nullifiers or output commitments are duplicated.
    #[error("private settlement fixed state slots contain duplicates")]
    DuplicateStateItem,
    /// Fixed input/output vectors do not have the closed protocol lengths.
    #[error(
        "private settlement fixed slots require 2 nullifiers and 3 outputs, got {nullifiers} and {outputs}"
    )]
    InvalidFixedSlotCount {
        /// Actual nullifier count.
        nullifiers: usize,
        /// Actual output or encrypted-output count.
        outputs: usize,
    },
    /// Fixed delta fields differ from the proof statement.
    #[error("private settlement delta does not match its proof statement")]
    DeltaStatementMismatch,
    /// One encrypted output is malformed or misaligned.
    #[error("private settlement encrypted output {index} is invalid")]
    InvalidEncryptedOutput {
        /// Fixed output index.
        index: usize,
    },
    /// Restricted-DA certificate has an invalid body, quorum, or wire shape.
    #[error("private settlement restricted-DA availability certificate is invalid")]
    InvalidAvailabilityCertificate,
    /// One provisional availability share is malformed or unauthenticated.
    #[error("private settlement restricted-DA availability share is invalid")]
    InvalidAvailabilityShare,
    /// A node auditor-view attestation has malformed or reserved fields.
    #[error("private settlement auditor view attestation is invalid")]
    InvalidAuditorViewAttestation,
    /// A node approval-acknowledgement attestation is malformed or reserved.
    #[error("private settlement audit approval acknowledgement attestation is invalid")]
    InvalidAuditApprovalAcknowledgementAttestation,
    /// Auditor-only plaintext has an invalid fixed shape, value balance, or dummy slot.
    #[error("private settlement auditor plaintext is invalid")]
    InvalidAuditPlaintext,
    /// Auditor-only plaintext differs from its exact public manifest leg.
    #[error("private settlement auditor plaintext binding mismatch")]
    AuditPlaintextBindingMismatch,
    /// Designated reimbursement plaintext does not open the public terms commitment.
    #[error("private settlement reimbursement terms commitment mismatch")]
    ReimbursementTermsMismatch,
    /// Auditor policy lifecycle fields are invalid.
    #[error("private settlement audit policy lifecycle is invalid")]
    InvalidAuditPolicyLifecycle,
    /// Auditor threshold or roster size is invalid.
    #[error("private settlement audit threshold is invalid")]
    InvalidAuditThreshold,
    /// Auditors are duplicated or not strictly ordered.
    #[error("private settlement auditors must be strictly ordered and unique")]
    NonCanonicalAuditorOrder,
    /// Purpose-specific auditor keys are reused.
    #[error("private settlement auditor keys must be unique")]
    DuplicateAuditorKey,
    /// Hybrid public encryption key is malformed.
    #[error("private settlement auditor hybrid public key is invalid")]
    InvalidHybridPublicKey,
    /// Policy self-digest is invalid.
    #[error("private settlement audit policy digest mismatch")]
    AuditPolicyDigestMismatch,
    /// Restricted pool route is universal or has a reserved incarnation.
    #[error("private settlement pool governance route is invalid")]
    InvalidPoolGovernanceRoute,
    /// Restricted pool governance revision or activation interval is invalid.
    #[error("private settlement pool governance lifecycle is invalid")]
    InvalidPoolGovernanceLifecycle,
    /// A required pool, salt, commitment, policy, or key-epoch field is reserved.
    #[error("private settlement pool governance binding is invalid")]
    InvalidPoolGovernanceBinding,
    /// Exact route, pool, asset, or salt does not open the governed commitment.
    #[error("private settlement pool governance asset binding mismatch")]
    PoolGovernanceAssetBindingMismatch,
    /// Restricted pool mapping names the wrong audit policy or key epoch.
    #[error("private settlement pool governance audit policy mismatch")]
    PoolGovernancePolicyMismatch,
    /// Restricted pool governance self-digest is invalid.
    #[error("private settlement pool governance digest mismatch")]
    PoolGovernanceDigestMismatch,
    /// Restricted pool mapping or its audit policy is inactive at the requested height.
    #[error("private settlement pool governance mapping is stale")]
    StalePoolGovernance,
    /// Capsule authenticated data does not match its policy.
    #[error("private settlement audit capsule binding mismatch")]
    AuditCapsuleBindingMismatch,
    /// Capsule ciphertext does not match its fixed padding class.
    #[error("private settlement audit capsule ciphertext is invalid")]
    InvalidAuditCapsuleCiphertext,
    /// Capsule recipients differ from the exact policy roster.
    #[error("private settlement audit capsule recipients do not match policy")]
    AuditCapsuleRecipientMismatch,
    /// One wrapped DEK has malformed KEM or AEAD material.
    #[error("private settlement wrapped DEK is invalid")]
    InvalidWrappedDek,
    /// Proof bytes are empty or exceed the profile bound.
    #[error("private settlement proof size is invalid")]
    InvalidProofSize,
    /// Proof statement ordinal does not identify a manifest leg.
    #[error("private settlement payload references an unknown leg")]
    UnknownLeg,
    /// Restricted payload does not match the public manifest or local policy.
    #[error("private settlement payload does not match manifest")]
    ManifestPayloadMismatch,
    /// Capsule digest differs from the proof statement.
    #[error("private settlement audit capsule digest mismatch")]
    AuditCapsuleDigestMismatch,
    /// Proof-byte digest differs from the committed delta.
    #[error("private settlement proof digest mismatch")]
    ProofDigestMismatch,
    /// Sidecar ticket, retention, length, or certificate does not match.
    #[error("private settlement sidecar availability mismatch")]
    SidecarAvailabilityMismatch,
    /// Auditor approval is outside the policy or approval validity interval.
    #[error("private settlement audit approval is stale or policy-inconsistent")]
    StaleAuditApproval,
    /// Approval signer is not in the governed local policy.
    #[error("private settlement approval signer is not an authorized auditor")]
    UnauthorizedAuditor,
    /// Auditor signature verification failed.
    #[error("private settlement auditor signature is invalid")]
    InvalidAuditSignature,
    /// Approval set is duplicated or not strictly ordered.
    #[error("private settlement approvals must be strictly ordered and unique")]
    NonCanonicalApprovalOrder,
    /// Approval threshold was not met.
    #[error("private settlement has {actual} approvals but requires {required}")]
    InsufficientAuditApprovals {
        /// Actual approval count.
        actual: usize,
        /// Governed threshold.
        required: u8,
    },
    /// Approval body differs from the exact proof, capsule, delta, or roots.
    #[error("private settlement audit approval binding mismatch")]
    AuditApprovalBindingMismatch,
    /// Four-validator authority record is malformed.
    #[error("private settlement committee authority is invalid")]
    InvalidCommitteeAuthority,
    /// Compact authority catalog is malformed or non-canonical.
    #[error("private settlement authority catalog is invalid")]
    InvalidAuthorityCatalog,
    /// Phase certificate bitmap or signature shape is malformed.
    #[error("private settlement phase certificate is invalid")]
    InvalidPhaseCertificate,
    /// One participant phase vote is malformed.
    #[error("private settlement phase vote is invalid")]
    InvalidPhaseVote,
    /// The complete all-Prepare barrier is missing or misaligned.
    #[error("private settlement Prepare barrier is invalid")]
    InvalidPrepareBarrier,
    /// Receipt count, height, or canonical structure is invalid.
    #[error("private settlement receipt shape is invalid")]
    InvalidReceiptShape,
    /// Receipt leg, authority, delta, or phase body does not match the manifest.
    #[error("private settlement receipt binding mismatch")]
    ReceiptBindingMismatch,
    /// Public receipt exceeds the carrier byte budget.
    #[error("private settlement receipt size {bytes} exceeds the protocol limit")]
    ReceiptTooLarge {
        /// Actual canonical receipt size.
        bytes: usize,
    },
    /// Abort receipt is malformed.
    #[error("private settlement abort receipt is invalid")]
    InvalidAbortReceipt,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_digest_helper_is_domain_and_length_separated() {
        let proof = b"atomic-private-settlement-proof";
        assert_eq!(
            private_settlement_proof_digest_v1(proof),
            private_settlement_proof_digest_v1(proof)
        );
        assert_ne!(private_settlement_proof_digest_v1(proof), Hash::new(proof));
        let mut suffixed = proof.to_vec();
        suffixed.push(0);
        assert_ne!(
            private_settlement_proof_digest_v1(proof),
            private_settlement_proof_digest_v1(&suffixed)
        );
    }
    use crate::block::BlockHeader;
    use crate::domain::DomainId;
    use crate::privacy::{PrivacyEncryptionKeyV1, PrivacyRecipientIdV1};
    use iroha_crypto::{Algorithm, HashOf, HybridKeyPair, KeyPair};

    fn network(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new([
            seed,
        ])))
    }

    fn hash(seed: u8) -> Hash {
        Hash::new([seed])
    }

    #[test]
    fn audit_payer_input_json_requires_explicit_dummy_domain() {
        let input = PrivateSettlementAuditPayerInputV1 {
            input_ordinal: 0,
            active: true,
            commitment: PrivacyCommitmentV1::new([1; 32]),
            nullifier: PrivacyNullifierV1::new([2; 32]),
            note_spending_authority: [3; 32],
            dummy_domain: None,
        };
        let value = norito::json::to_value(&input).expect("encode payer input JSON");
        assert_eq!(value.get("dummy_domain"), Some(&norito::json::Value::Null));
        let decoded = norito::json::from_value::<PrivateSettlementAuditPayerInputV1>(value.clone())
            .expect("explicit null dummy domain decodes");
        assert_eq!(decoded, input);

        let mut omitted = value;
        omitted
            .as_object_mut()
            .expect("payer input is a JSON object")
            .remove("dummy_domain");
        let error = norito::json::from_value::<PrivateSettlementAuditPayerInputV1>(omitted)
            .expect_err("omitted dummy domain must reject");
        assert!(error.to_string().contains("missing field `dummy_domain`"));
    }

    #[test]
    fn audit_note_opening_json_requires_explicit_dummy_domain() {
        let opening = active_opening(4, 9);
        let value = norito::json::to_value(&opening).expect("encode audit note opening JSON");
        assert_eq!(value.get("dummy_domain"), Some(&norito::json::Value::Null));
        let decoded =
            norito::json::from_value::<PrivateSettlementAuditNoteOpeningV1>(value.clone())
                .expect("explicit null dummy domain decodes");
        assert_eq!(decoded, opening);

        let mut omitted = value;
        omitted
            .as_object_mut()
            .expect("audit note opening is a JSON object")
            .remove("dummy_domain");
        let error = norito::json::from_value::<PrivateSettlementAuditNoteOpeningV1>(omitted)
            .expect_err("omitted dummy domain must reject");
        assert!(error.to_string().contains("missing field `dummy_domain`"));
    }

    fn route(dataspace: u64) -> PrivateSettlementRouteV1 {
        PrivateSettlementRouteV1 {
            dataspace_id: DataSpaceId::new(dataspace),
            lane_id: LaneId::new(u32::try_from(dataspace).expect("fixture lane fits u32")),
            lane_incarnation: Hash::new(dataspace.to_le_bytes()),
        }
    }

    fn manifest(count: usize) -> AtomicPrivateSettlementV1 {
        let sponsor_key = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
        let mut manifest = AtomicPrivateSettlementV1 {
            version: AtomicPrivateSettlementV1::VERSION,
            network_id: network(1),
            bundle_id: hash(2),
            authority_context_height: 10,
            expiry_height: 100,
            sponsor: AccountId::new(sponsor_key.public_key().clone()),
            public_fee_intent: FeePaymentIntent::authority(Vec::new(), None),
            fee_intent_digest: hash(3),
            reimbursement_terms_commitment: hash(4),
            reimbursement_leg_ordinal: 0,
            legs: (0..count)
                .map(|index| {
                    let ordinal = u8::try_from(index).expect("fixture ordinal fits u8");
                    PrivateSettlementLegCommitmentV1 {
                        ordinal,
                        route: route(u64::try_from(index + 1).expect("fixture route fits u64")),
                        pool_id: PrivacyPoolIdV1::new([ordinal.saturating_add(1); 32]),
                        asset_binding_commitment: hash(ordinal.saturating_add(40)),
                        audit_policy_digest: hash(ordinal.saturating_add(70)),
                        payload_digest: hash(ordinal.saturating_add(90)),
                        availability_certificate_digest: hash(ordinal.saturating_add(100)),
                        delta_digest: hash(ordinal.saturating_add(110)),
                    }
                })
                .collect(),
        };
        manifest.fee_intent_digest = manifest
            .computed_fee_intent_digest()
            .expect("fixture fee intent hashes");
        manifest.bundle_id = manifest
            .computed_bundle_id()
            .expect("fixture bundle hashes");
        manifest
    }

    fn measured_bytes32(label: &[u8], index: usize, slot: u8) -> [u8; 32] {
        let digest = Hash::new_from_chunks(&[
            b"private-settlement-wire-size-fixture-v1",
            label,
            &u64::try_from(index)
                .expect("fixture index fits u64")
                .to_le_bytes(),
            &[slot],
        ]);
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(digest.as_ref());
        bytes
    }

    fn measured_validator_material() -> (Vec<PeerId>, Vec<Vec<u8>>) {
        let keypairs = (0_u8..4)
            .map(|index| {
                KeyPair::from_seed(
                    vec![0xB0_u8.saturating_add(index); 32],
                    Algorithm::BlsNormal,
                )
            })
            .collect::<Vec<_>>();
        let validators = keypairs
            .iter()
            .map(|key| PeerId::from(key.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_pops = keypairs
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture BLS proof of possession")
            })
            .collect::<Vec<_>>();
        (validators, validator_pops)
    }

    fn measured_authority(
        route: PrivateSettlementRouteV1,
        validators: &[PeerId],
        validator_pops: &[Vec<u8>],
    ) -> PrivateSettlementCommitteeAuthorityV1 {
        let validators = validators.to_vec();
        PrivateSettlementCommitteeAuthorityV1 {
            route,
            validator_set_hash: HashOf::new(&validators),
            validators,
            validator_pops: validator_pops.to_vec(),
        }
    }

    fn measured_delta(
        manifest: &AtomicPrivateSettlementV1,
        index: usize,
    ) -> PrivateSettlementDeltaV1 {
        let leg = &manifest.legs[index];
        let output_commitments = (0_u8..3)
            .map(|slot| PrivacyCommitmentV1::new(measured_bytes32(b"commitment", index, slot)))
            .collect::<Vec<_>>();
        let encrypted_outputs = output_commitments
            .iter()
            .copied()
            .enumerate()
            .map(|(slot, commitment)| {
                let slot = u8::try_from(slot).expect("fixture slot fits u8");
                let mut ciphertext =
                    vec![slot.saturating_add(1); PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1];
                ciphertext[..4].copy_from_slice(b"IPNE");
                PrivacyEncryptedOutputV1 {
                    recipient: PrivacyRecipientIdV1::new(measured_bytes32(
                        b"recipient",
                        index,
                        slot,
                    )),
                    ephemeral_public_key: PrivacyEncryptionKeyV1::new(measured_bytes32(
                        b"encryption-key",
                        index,
                        slot,
                    )),
                    commitment,
                    ciphertext,
                }
            })
            .collect::<Vec<_>>();
        PrivateSettlementDeltaV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            bundle_id: manifest.bundle_id,
            leg_ordinal: leg.ordinal,
            route: leg.route,
            pool_id: leg.pool_id,
            asset_binding_commitment: leg.asset_binding_commitment,
            old_root: PrivacyRootV1::new(measured_bytes32(b"old-root", index, 0)),
            new_root: PrivacyRootV1::new(measured_bytes32(b"new-root", index, 0)),
            old_epoch: 7,
            new_epoch: 8,
            nullifiers: (0_u8..2)
                .map(|slot| PrivacyNullifierV1::new(measured_bytes32(b"nullifier", index, slot)))
                .collect(),
            output_commitments,
            encrypted_outputs,
            statement_digest: Hash::prehashed(measured_bytes32(b"statement", index, 0)),
            proof_digest: Hash::prehashed(measured_bytes32(b"proof", index, 0)),
            capsule_digest: Hash::prehashed(measured_bytes32(b"capsule", index, 0)),
            audit_policy_digest: leg.audit_policy_digest,
            audit_key_epoch: 3,
        }
    }

    fn measured_receipt(count: usize) -> PrivateSettlementReceiptV1 {
        let mut manifest = manifest(count);
        let deltas = (0..count)
            .map(|index| measured_delta(&manifest, index))
            .collect::<Vec<_>>();
        for (leg, delta) in manifest.legs.iter_mut().zip(&deltas) {
            leg.delta_digest = delta.digest().expect("fixture delta hashes");
        }
        manifest.validate().expect("measured manifest validates");
        let manifest_digest = manifest.manifest_digest().expect("fixture manifest hashes");
        let (validators, validator_pops) = measured_validator_material();
        let authorities = manifest
            .legs
            .iter()
            .map(|leg| measured_authority(leg.route, &validators, &validator_pops))
            .collect::<Vec<_>>();
        let authority_catalog =
            PrivateSettlementAuthorityCatalogV1::from_leg_authorities(&manifest, &authorities)
                .expect("fixture authority catalog compacts");
        let prepared_bundle_digest = hash(0xE1);
        let legs = deltas
            .into_iter()
            .zip(&authorities)
            .enumerate()
            .map(|(index, (delta, authority))| {
                let ordinal = u8::try_from(index).expect("fixture ordinal fits u8");
                let common = PrivateSettlementPhaseBodyV1 {
                    network_id: manifest.network_id,
                    bundle_id: manifest.bundle_id,
                    manifest_digest,
                    leg_ordinal: ordinal,
                    route: manifest.legs[index].route,
                    delta_digest: delta.digest().expect("fixture delta hashes"),
                    authority_digest: authority.digest().expect("fixture authority hashes"),
                    prepared_bundle_digest: Hash::prehashed([0; Hash::LENGTH]),
                    phase: PrivateSettlementPhaseV1::Prepare,
                    authority_context_height: manifest.authority_context_height,
                    expiry_height: manifest.expiry_height,
                };
                let prepare = PrivateSettlementPhaseCertificateV1 {
                    body: common,
                    authority_catalog_index: ordinal,
                    signers_bitmap: 0b0111,
                    aggregate_signature: vec![0xA1; PRIVATE_SETTLEMENT_BLS_BYTES_V1],
                };
                let mut commit_body = common;
                commit_body.phase = PrivateSettlementPhaseV1::Commit;
                commit_body.prepared_bundle_digest = prepared_bundle_digest;
                let commit = PrivateSettlementPhaseCertificateV1 {
                    body: commit_body,
                    authority_catalog_index: ordinal,
                    signers_bitmap: 0b1011,
                    aggregate_signature: vec![0xA2; PRIVATE_SETTLEMENT_BLS_BYTES_V1],
                };
                PrivateSettlementLegReceiptV1 {
                    delta,
                    prepare,
                    commit,
                }
            })
            .collect();
        PrivateSettlementReceiptV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            manifest,
            authority_catalog,
            legs,
            finalized_height: 50,
        }
    }

    fn policy(dataspace: DataSpaceId) -> (PrivateSettlementAuditPolicyV1, Vec<KeyPair>) {
        let signing_keys = vec![
            KeyPair::from_seed(vec![0x61; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![0x62; 32], Algorithm::Ed25519),
        ];
        let mut auditors = Vec::new();
        for (index, signing) in signing_keys.iter().enumerate() {
            let mut rng = iroha_crypto::rng_from_seed_slice(&[
                0xA0_u8.saturating_add(u8::try_from(index).expect("fixture index fits u8"))
            ]);
            let encryption = HybridKeyPair::generate(&mut rng).expect("hybrid fixture key");
            auditors.push(PrivateSettlementAuditorV1 {
                auditor_id: AccountId::new(signing.public_key().clone()),
                signing_key: signing.public_key().clone(),
                encryption_key: PrivateSettlementHybridPublicKeyV1::from_hybrid(
                    encryption.public(),
                ),
            });
        }
        auditors.sort_by(|left, right| left.auditor_id.cmp(&right.auditor_id));
        let body = PrivateSettlementAuditPolicyBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            dataspace_id: dataspace,
            policy_id: hash(7),
            revision: 1,
            key_epoch: 1,
            activation_height: 5,
            retirement_height: Some(500),
            min_approvals: 1,
            auditors,
        };
        (
            PrivateSettlementAuditPolicyV1::new(body).expect("fixture policy is valid"),
            signing_keys,
        )
    }

    fn pool_governance_fixture() -> (
        PrivateSettlementAuditPolicyV1,
        PrivateSettlementPoolGovernanceV1,
        AssetDefinitionId,
    ) {
        let (policy, _) = policy(DataSpaceId::new(1));
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("bank-a", "regulated").expect("fixture domain"),
            "cbdc".parse().expect("fixture asset name"),
        );
        let governance = PrivateSettlementPoolGovernanceV1::from_restricted_mapping(
            route(1),
            PrivacyPoolIdV1::new([0x91; 32]),
            asset_definition_id.clone(),
            [0xA1; 32],
            &policy,
            PrivateSettlementPoolGovernanceLifecycleV1 {
                governance_revision: 1,
                activation_height: 10,
                retirement_height: Some(400),
            },
        )
        .expect("fixture pool governance is valid");
        (policy, governance, asset_definition_id)
    }

    fn active_opening(seed: u8, value: u128) -> PrivateSettlementAuditNoteOpeningV1 {
        PrivateSettlementAuditNoteOpeningV1 {
            active: true,
            commitment: PrivacyCommitmentV1::new([seed; 32]),
            value,
            spending_authority: [seed.wrapping_add(1); 32],
            rho: [seed.wrapping_add(2); 32],
            blinding: [seed.wrapping_add(3); 32],
            memo_digest: [seed.wrapping_add(4); 32],
            dummy_domain: None,
        }
    }

    fn dummy_opening(seed: u8) -> PrivateSettlementAuditNoteOpeningV1 {
        PrivateSettlementAuditNoteOpeningV1 {
            active: false,
            commitment: PrivacyCommitmentV1::new([seed; 32]),
            value: 0,
            spending_authority: [seed.wrapping_add(2); 32],
            rho: [seed.wrapping_add(3); 32],
            blinding: [seed.wrapping_add(4); 32],
            memo_digest: [seed.wrapping_add(5); 32],
            dummy_domain: Some(hash(seed.wrapping_add(1))),
        }
    }

    fn placeholder_view_key_authorization(
        signing: &KeyPair,
    ) -> PrivateSettlementAuditViewKeyAuthorizationV1 {
        let body = PrivateSettlementAuditViewKeyAuthorizationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            purpose: Hash::new(OUTPUT_VIEW_KEY_AUTHORIZATION_DOMAIN_V1),
            network_id: network(1),
            bundle_id: hash(1),
            leg_ordinal: 0,
            route: route(1),
            output_ordinal: 0,
            role: PrivateSettlementAuditOutputRoleV1::SettlementRecipient,
            authorized_account: AccountId::new(signing.public_key().clone()),
            recipient_view_key: [1; 32],
            output_active: true,
            note_spending_authority: [2; 32],
            expiry_height: 1,
        };
        PrivateSettlementAuditViewKeyAuthorizationV1::new(
            body.clone(),
            vec![PrivateSettlementAuditViewKeySignatureV1::new(
                signing.public_key().clone(),
                SignatureOf::try_new(signing.private_key(), &body)
                    .expect("placeholder authorization signs"),
            )],
        )
    }

    fn placeholder_payer_authorization(
        signing: &KeyPair,
    ) -> PrivateSettlementAuditPayerAuthorizationV1 {
        let body = PrivateSettlementAuditPayerAuthorizationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            purpose: Hash::new(PAYER_INPUT_AUTHORIZATION_DOMAIN_V1),
            network_id: network(1),
            bundle_id: hash(1),
            leg_ordinal: 0,
            route: route(1),
            payer: AccountId::new(signing.public_key().clone()),
            expiry_height: 1,
            inputs: vec![
                PrivateSettlementAuditPayerInputV1 {
                    input_ordinal: 0,
                    active: true,
                    commitment: PrivacyCommitmentV1::new([1; 32]),
                    nullifier: PrivacyNullifierV1::new([2; 32]),
                    note_spending_authority: [3; 32],
                    dummy_domain: None,
                },
                PrivateSettlementAuditPayerInputV1 {
                    input_ordinal: 1,
                    active: false,
                    commitment: PrivacyCommitmentV1::new([4; 32]),
                    nullifier: PrivacyNullifierV1::new([5; 32]),
                    note_spending_authority: [6; 32],
                    dummy_domain: Some(hash(7)),
                },
            ],
        };
        PrivateSettlementAuditPayerAuthorizationV1::new(
            body.clone(),
            vec![PrivateSettlementAuditPayerSignatureV1::new(
                signing.public_key().clone(),
                SignatureOf::try_new(signing.private_key(), &body)
                    .expect("placeholder payer authorization signs"),
            )],
        )
    }

    fn authorize_payer_inputs(
        plaintext: &mut PrivateSettlementAuditPlaintextV1,
        nullifiers: &[PrivacyNullifierV1],
        signer: &KeyPair,
    ) {
        let body = plaintext
            .payer_authorization_body(nullifiers)
            .expect("fixture payer authorization body");
        plaintext.payer_authorization = PrivateSettlementAuditPayerAuthorizationV1::new(
            body.clone(),
            vec![PrivateSettlementAuditPayerSignatureV1::new(
                signer.public_key().clone(),
                SignatureOf::try_new(signer.private_key(), &body)
                    .expect("fixture payer authorization signs"),
            )],
        );
    }

    fn authorize_output_view_keys(
        plaintext: &mut PrivateSettlementAuditPlaintextV1,
        signers: [&KeyPair; PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1],
    ) {
        for (index, signer) in signers.into_iter().enumerate() {
            let body = plaintext
                .output_view_key_authorization_body(index)
                .expect("fixture authorization body");
            plaintext.outputs[index].view_key_authorization =
                PrivateSettlementAuditViewKeyAuthorizationV1::new(
                    body.clone(),
                    vec![PrivateSettlementAuditViewKeySignatureV1::new(
                        signer.public_key().clone(),
                        SignatureOf::try_new(signer.private_key(), &body)
                            .expect("fixture authorization signs"),
                    )],
                );
        }
    }

    fn audit_plaintext_fixture() -> (AtomicPrivateSettlementV1, PrivateSettlementAuditPlaintextV1) {
        let mut manifest = manifest(2);
        let sponsor = KeyPair::from_seed(vec![0x51; 32], Algorithm::Ed25519);
        let payer = KeyPair::from_seed(vec![0xB1; 32], Algorithm::Ed25519);
        let recipient = KeyPair::from_seed(vec![0xB2; 32], Algorithm::Ed25519);
        let asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("bank-a", "regulated").expect("fixture domain"),
            "cbdc".parse().expect("fixture asset name"),
        );
        let mut plaintext = PrivateSettlementAuditPlaintextV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: manifest.network_id,
            bundle_id: manifest.bundle_id,
            leg_ordinal: 0,
            route: manifest.legs[0].route,
            pool_id: manifest.legs[0].pool_id,
            payer: AccountId::new(payer.public_key().clone()),
            payer_authorization: placeholder_payer_authorization(&payer),
            recipient: AccountId::new(recipient.public_key().clone()),
            sponsor: manifest.sponsor.clone(),
            asset_definition_id,
            asset_binding_salt: [0xC1; 32],
            amount: 100,
            sponsor_reimbursement_amount: 20,
            fee_intent_digest: manifest.fee_intent_digest,
            settlement_expiry_height: manifest.expiry_height,
            reimbursement_terms_salt: [0xC4; 32],
            memo: b"invoice-2026-08".to_vec(),
            policy_references: {
                let mut references = vec![hash(0xC2), hash(0xC3)];
                references.sort_unstable();
                references
            },
            inputs: vec![active_opening(0xD0, 120), dummy_opening(0xD1)],
            outputs: vec![
                PrivateSettlementAuditOutputV1 {
                    role: PrivateSettlementAuditOutputRoleV1::SettlementRecipient,
                    recipient_view_key: [0xE1; 32],
                    view_key_authorization: placeholder_view_key_authorization(&recipient),
                    encryption_opening: PrivateSettlementAuditEncryptionOpeningV1 {
                        ephemeral_secret: [0xF1; 32],
                    },
                    note: active_opening(0xD2, 100),
                },
                PrivateSettlementAuditOutputV1 {
                    role: PrivateSettlementAuditOutputRoleV1::PayerChange,
                    recipient_view_key: [0xE2; 32],
                    view_key_authorization: placeholder_view_key_authorization(&payer),
                    encryption_opening: PrivateSettlementAuditEncryptionOpeningV1 {
                        ephemeral_secret: [0xF2; 32],
                    },
                    note: dummy_opening(0xD3),
                },
                PrivateSettlementAuditOutputV1 {
                    role: PrivateSettlementAuditOutputRoleV1::SponsorReimbursement,
                    recipient_view_key: [0xE3; 32],
                    view_key_authorization: placeholder_view_key_authorization(&sponsor),
                    encryption_opening: PrivateSettlementAuditEncryptionOpeningV1 {
                        ephemeral_secret: [0xF3; 32],
                    },
                    note: active_opening(0xD4, 20),
                },
            ],
        };
        manifest.legs[0].asset_binding_commitment = plaintext
            .asset_binding_commitment()
            .expect("fixture asset binding");
        manifest.reimbursement_terms_commitment = plaintext
            .reimbursement_terms_commitment()
            .expect("fixture reimbursement terms");
        manifest.bundle_id = manifest.computed_bundle_id().expect("fixture bundle id");
        plaintext.bundle_id = manifest.bundle_id;
        authorize_payer_inputs(
            &mut plaintext,
            &[
                PrivacyNullifierV1::new([0xA1; 32]),
                PrivacyNullifierV1::new([0xA2; 32]),
            ],
            &payer,
        );
        authorize_output_view_keys(&mut plaintext, [&recipient, &payer, &sponsor]);
        (manifest, plaintext)
    }

    #[test]
    fn manifest_enforces_two_through_255_canonical_legs() {
        assert_eq!(
            manifest(1).validate(),
            Err(PrivateSettlementValidationError::ParticipantCount { count: 1 })
        );
        manifest(2).validate().expect("two legs are admitted");
        manifest(255).validate().expect("255 legs are admitted");

        let mut reordered = manifest(3);
        reordered.legs.swap(0, 1);
        assert_eq!(
            reordered.validate(),
            Err(PrivateSettlementValidationError::NonCanonicalOrdinal {
                index: 0,
                actual: 1,
            })
        );

        let mut duplicate_dataspace = manifest(2);
        duplicate_dataspace.legs[1].route.dataspace_id =
            duplicate_dataspace.legs[0].route.dataspace_id;
        duplicate_dataspace.bundle_id = duplicate_dataspace
            .computed_bundle_id()
            .expect("fixture bundle hashes");
        assert_eq!(
            duplicate_dataspace.validate(),
            Err(PrivateSettlementValidationError::DuplicateDataspace)
        );
    }

    #[test]
    fn auditor_plaintext_is_fixed_balanced_bound_and_redacted() {
        let (manifest, plaintext) = audit_plaintext_fixture();
        plaintext.validate().expect("valid fixed plaintext");
        plaintext
            .validate_against_manifest(&manifest)
            .expect("plaintext binds exact manifest leg");
        let commitment = plaintext.commitment().expect("plaintext commitment");
        let reimbursement = plaintext
            .reimbursement_terms_commitment()
            .expect("reimbursement terms commitment");
        let one_carrier_reimbursement = canonical_hash(
            REIMBURSEMENT_TERMS_COMMITMENT_DOMAIN_V1,
            &plaintext.reimbursement_terms_material(1),
        )
        .expect("one-carrier reimbursement terms commitment");
        assert_eq!(PRIVATE_SETTLEMENT_SUCCESS_FEE_BEARING_CARRIERS_V1, 2);
        assert_ne!(
            reimbursement, one_carrier_reimbursement,
            "reimbursement terms must bind both fee-bearing success carriers"
        );
        let mut changed = plaintext.clone();
        changed.memo.push(b'!');
        assert_ne!(
            commitment,
            changed.commitment().expect("changed plaintext commitment")
        );
        let mut changed_output_secret = plaintext.clone();
        changed_output_secret.outputs[0].note.blinding[0] ^= 1;
        assert_ne!(
            commitment,
            changed_output_secret
                .commitment()
                .expect("changed output opening commitment")
        );
        let mut changed_derived_output = plaintext.clone();
        changed_derived_output.outputs[0].note.memo_digest[0] ^= 1;
        changed_derived_output.outputs[0].note.commitment = PrivacyCommitmentV1::new([0xFA; 32]);
        assert_eq!(
            commitment,
            changed_derived_output
                .commitment()
                .expect("derived output fields are excluded from the projection")
        );
        let mut changed_authorization = plaintext.clone();
        changed_authorization.outputs[0]
            .view_key_authorization
            .body
            .expiry_height -= 1;
        assert_ne!(
            commitment,
            changed_authorization
                .commitment()
                .expect("changed authorization material commitment")
        );
        let mut changed_payer_authorization = plaintext.clone();
        changed_payer_authorization.payer_authorization.body.inputs[0].nullifier =
            PrivacyNullifierV1::new([0xFB; 32]);
        assert_ne!(
            commitment,
            changed_payer_authorization
                .commitment()
                .expect("changed payer authorization commitment")
        );
        let mut changed_encryption_opening = plaintext.clone();
        changed_encryption_opening.outputs[0]
            .encryption_opening
            .ephemeral_secret[0] ^= 1;
        assert_ne!(
            commitment,
            changed_encryption_opening
                .commitment()
                .expect("changed encryption opening commitment")
        );
        assert_eq!(
            format!("{plaintext:?}"),
            "PrivateSettlementAuditPlaintextV1(<redacted>)"
        );
        assert_eq!(
            format!("{:?}", plaintext.outputs[0].view_key_authorization),
            "PrivateSettlementAuditViewKeyAuthorizationV1(<redacted>)"
        );
        assert_eq!(
            format!("{:?}", plaintext.payer_authorization),
            "PrivateSettlementAuditPayerAuthorizationV1(<redacted>)"
        );
        assert_eq!(
            format!("{:?}", plaintext.outputs[0].encryption_opening),
            "PrivateSettlementAuditEncryptionOpeningV1(<redacted>)"
        );
        let encoded = norito::encode_canonical(&plaintext).expect("audit plaintext encodes");
        let decoded = norito::decode_canonical::<PrivateSettlementAuditPlaintextV1>(&encoded)
            .expect("audit plaintext decodes canonically");
        assert_eq!(decoded, plaintext);

        let mut unbalanced = plaintext.clone();
        unbalanced.outputs[0].note.value += 1;
        assert_eq!(
            unbalanced.validate(),
            Err(PrivateSettlementValidationError::InvalidAuditPlaintext)
        );
        let mut literal_asset_substitution = plaintext;
        literal_asset_substitution.asset_definition_id = AssetDefinitionId::derive_from_components(
            DomainId::try_new("bank-a", "regulated").expect("fixture domain"),
            "other".parse().expect("fixture asset name"),
        );
        assert_eq!(
            literal_asset_substitution.validate_against_manifest(&manifest),
            Err(PrivateSettlementValidationError::AuditPlaintextBindingMismatch)
        );
    }

    #[test]
    fn proof_binding_excludes_post_proof_artifacts_but_manifest_digest_binds_them() {
        let original = manifest(2);
        let mut changed = original.clone();
        changed.legs[0].payload_digest = hash(250);
        changed.legs[0].availability_certificate_digest = hash(251);
        changed.legs[0].delta_digest = hash(252);
        changed
            .validate()
            .expect("post-proof manifest remains valid");
        assert_eq!(
            original.computed_bundle_id().expect("bundle hashes"),
            changed.computed_bundle_id().expect("bundle hashes")
        );
        assert_eq!(
            original
                .proof_binding_digest()
                .expect("proof binding hashes"),
            changed
                .proof_binding_digest()
                .expect("proof binding hashes")
        );
        assert_ne!(
            original.manifest_digest().expect("manifest hashes"),
            changed.manifest_digest().expect("manifest hashes")
        );
    }

    #[test]
    fn proof_binding_commits_every_ordered_settlement_intent_field() {
        let original = manifest(2);
        let original_digest = original
            .proof_binding_digest()
            .expect("proof binding hashes");
        let assert_changed = |mut changed: AtomicPrivateSettlementV1| {
            changed.bundle_id = changed.computed_bundle_id().expect("bundle hashes");
            changed.validate().expect("changed manifest remains valid");
            assert_ne!(
                original_digest,
                changed
                    .proof_binding_digest()
                    .expect("proof binding hashes")
            );
        };

        let mut changed = original.clone();
        changed.network_id = network(2);
        assert_changed(changed);

        let mut changed = original.clone();
        changed.authority_context_height += 1;
        assert_changed(changed);

        let mut changed = original.clone();
        changed.expiry_height += 1;
        assert_changed(changed);

        let mut changed = original.clone();
        let sponsor_key = KeyPair::from_seed(vec![0x52; 32], Algorithm::Ed25519);
        changed.sponsor = AccountId::new(sponsor_key.public_key().clone());
        assert_changed(changed);

        let mut changed = original.clone();
        changed.public_fee_intent =
            FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(1));
        changed.fee_intent_digest = changed
            .computed_fee_intent_digest()
            .expect("fee intent hashes");
        assert_changed(changed);

        let mut changed = original.clone();
        changed.reimbursement_terms_commitment = hash(253);
        assert_changed(changed);

        let mut changed = original.clone();
        changed.reimbursement_leg_ordinal = 1;
        assert_changed(changed);

        let mut changed = original.clone();
        changed.legs[0].route.lane_id = LaneId::new(99);
        assert_changed(changed);

        let mut changed = original.clone();
        changed.legs[0].pool_id = PrivacyPoolIdV1::new([0xF1; 32]);
        assert_changed(changed);

        let mut changed = original.clone();
        changed.legs[0].asset_binding_commitment = hash(254);
        assert_changed(changed);

        let mut changed = original;
        changed.legs[0].audit_policy_digest = hash(255);
        assert_changed(changed);
    }

    #[test]
    fn carrier_and_receipt_wire_sizes_fit_protocol_limit_through_255_legs() {
        let mut previous_receipt_bytes = 0;
        for count in [2, 3, 4, 8, 16, 17, 255] {
            let receipt = measured_receipt(count);
            receipt
                .validate_shape()
                .expect("measured receipt validates");
            let receipt_bytes = norito::encode_canonical(&receipt)
                .expect("measured receipt encodes")
                .len();
            let carrier = PrivateSettlementCommitBundleV1 {
                version: receipt.version,
                manifest: receipt.manifest.clone(),
                authority_catalog: receipt.authority_catalog.clone(),
                legs: receipt.legs.clone(),
            };
            let carrier_bytes = carrier
                .canonical_carrier_bytes_len()
                .expect("measured carrier encodes");
            assert_eq!(
                receipt
                    .canonical_carrier_bytes_len()
                    .expect("receipt projects the measured carrier"),
                carrier_bytes
            );
            let instruction =
                crate::isi::private_settlement::FinalizeAtomicPrivateSettlementV1::new(
                    carrier.clone(),
                );
            let boxed = crate::isi::InstructionBox::from(instruction);
            assert_eq!(
                norito::encode_canonical(&boxed)
                    .expect("boxed carrier instruction encodes")
                    .len(),
                carrier_bytes
            );
            eprintln!(
                "atomic-private-settlement wire size: legs={count} receipt_bytes={receipt_bytes} carrier_bytes={carrier_bytes}"
            );
            assert!(
                receipt_bytes <= PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1,
                "{count}-leg receipt is {receipt_bytes} bytes"
            );
            assert!(
                carrier_bytes <= PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1,
                "{count}-leg carrier is {carrier_bytes} bytes"
            );
            assert!(receipt_bytes > previous_receipt_bytes);
            previous_receipt_bytes = receipt_bytes;
        }
    }

    #[test]
    fn audit_policy_requires_canonical_distinct_purpose_keys() {
        let (policy, _) = policy(DataSpaceId::new(1));
        policy.validate().expect("fixture policy is valid");

        let mut duplicate = policy.clone();
        duplicate.body.auditors[1].signing_key = duplicate.body.auditors[0].signing_key.clone();
        duplicate.policy_digest =
            canonical_hash(AUDIT_POLICY_DIGEST_DOMAIN_V1, &duplicate.body).expect("policy hashes");
        assert_eq!(
            duplicate.validate(),
            Err(PrivateSettlementValidationError::DuplicateAuditorKey)
        );
    }

    #[test]
    fn pool_governance_roundtrips_and_opens_exact_restricted_mapping() {
        let (policy, governance, asset_definition_id) = pool_governance_fixture();
        governance.validate().expect("governance validates");
        governance
            .validate_against_policy_at(&policy, 10)
            .expect("policy is exact and active");
        assert!(governance.is_active_at(10));
        assert!(!governance.is_active_at(400));
        governance
            .validate_asset_opening(
                route(1),
                PrivacyPoolIdV1::new([0x91; 32]),
                &asset_definition_id,
                [0xA1; 32],
            )
            .expect("restricted opening matches");
        assert_eq!(
            governance.body.audit_policy_digest,
            policy
                .computed_policy_digest()
                .expect("policy digest recomputes")
        );
        assert_eq!(
            governance.body.asset_binding_commitment,
            governance
                .body
                .computed_asset_binding_commitment()
                .expect("asset binding recomputes")
        );
        assert_eq!(
            governance.governance_digest,
            governance
                .computed_governance_digest()
                .expect("governance digest recomputes")
        );

        let encoded = norito::encode_canonical(&governance).expect("governance encodes");
        let decoded = norito::decode_canonical::<PrivateSettlementPoolGovernanceV1>(&encoded)
            .expect("governance decodes canonically");
        assert_eq!(decoded, governance);
        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&governance).expect("governance JSON encodes");
            let decoded_json: PrivateSettlementPoolGovernanceV1 =
                norito::json::from_json(&json).expect("governance JSON decodes");
            assert_eq!(decoded_json, governance);
        }
        assert_eq!(
            format!("{governance:?}"),
            "PrivateSettlementPoolGovernanceV1(<restricted>)"
        );
        assert_eq!(
            format!("{:?}", governance.body),
            "PrivateSettlementPoolGovernanceBodyV1(<restricted>)"
        );
    }

    #[test]
    fn pool_governance_rejects_asset_salt_route_and_pool_substitution() {
        let (_, governance, asset_definition_id) = pool_governance_fixture();
        let expected_error =
            Err(PrivateSettlementValidationError::PoolGovernanceAssetBindingMismatch);

        let wrong_asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("bank-a", "regulated").expect("fixture domain"),
            "other".parse().expect("fixture asset name"),
        );
        assert_eq!(
            governance.validate_asset_opening(
                route(1),
                governance.body.pool_id,
                &wrong_asset,
                governance.body.asset_binding_salt,
            ),
            expected_error
        );

        let mut wrong_salt = governance.body.asset_binding_salt;
        wrong_salt[0] ^= 1;
        assert_eq!(
            governance.validate_asset_opening(
                route(1),
                governance.body.pool_id,
                &asset_definition_id,
                wrong_salt,
            ),
            expected_error
        );

        let mut wrong_route = route(1);
        wrong_route.lane_incarnation = hash(0xE1);
        assert_eq!(
            governance.validate_asset_opening(
                wrong_route,
                governance.body.pool_id,
                &asset_definition_id,
                governance.body.asset_binding_salt,
            ),
            expected_error
        );
        assert_ne!(
            governance.body.asset_binding_commitment,
            private_settlement_asset_binding_commitment_v1(
                wrong_route,
                governance.body.pool_id,
                &asset_definition_id,
                governance.body.asset_binding_salt,
            )
            .expect("substituted route hashes")
        );

        assert_eq!(
            governance.validate_asset_opening(
                route(1),
                PrivacyPoolIdV1::new([0x92; 32]),
                &asset_definition_id,
                governance.body.asset_binding_salt,
            ),
            expected_error
        );
    }

    #[test]
    fn pool_governance_rejects_wrong_policy_epoch_and_stale_lifecycle() {
        let (policy, governance, _) = pool_governance_fixture();

        let mut wrong_policy_body = policy.body.clone();
        wrong_policy_body.policy_id = hash(0xE2);
        let wrong_policy = PrivateSettlementAuditPolicyV1::new(wrong_policy_body)
            .expect("substituted policy remains structural");
        assert_eq!(
            governance.validate_against_policy_at(&wrong_policy, 10),
            Err(PrivateSettlementValidationError::PoolGovernancePolicyMismatch)
        );

        let mut wrong_epoch_body = governance.body.clone();
        wrong_epoch_body.audit_key_epoch += 1;
        let wrong_epoch = PrivateSettlementPoolGovernanceV1::new(wrong_epoch_body)
            .expect("substituted epoch remains structural");
        assert_eq!(
            wrong_epoch.validate_against_policy_at(&policy, 10),
            Err(PrivateSettlementValidationError::PoolGovernancePolicyMismatch)
        );

        let mut wrong_digest_body = governance.body.clone();
        wrong_digest_body.audit_policy_digest = hash(0xE3);
        let wrong_digest = PrivateSettlementPoolGovernanceV1::new(wrong_digest_body)
            .expect("substituted policy digest remains structural");
        assert_eq!(
            wrong_digest.validate_against_policy_at(&policy, 10),
            Err(PrivateSettlementValidationError::PoolGovernancePolicyMismatch)
        );

        assert_eq!(
            governance.validate_against_policy_at(&policy, 9),
            Err(PrivateSettlementValidationError::StalePoolGovernance)
        );
        assert_eq!(
            governance.validate_against_policy_at(&policy, 400),
            Err(PrivateSettlementValidationError::StalePoolGovernance)
        );

        let mut outlives_policy_body = governance.body.clone();
        outlives_policy_body.lifecycle.retirement_height = None;
        let outlives_policy = PrivateSettlementPoolGovernanceV1::new(outlives_policy_body)
            .expect("open-ended mapping remains structural");
        assert_eq!(
            outlives_policy.validate_against_policy_at(&policy, 10),
            Err(PrivateSettlementValidationError::InvalidPoolGovernanceLifecycle)
        );

        let mut invalid_lifecycle = governance.clone().body;
        invalid_lifecycle.lifecycle.activation_height = 0;
        assert_eq!(
            PrivateSettlementPoolGovernanceV1::new(invalid_lifecycle),
            Err(PrivateSettlementValidationError::InvalidPoolGovernanceLifecycle)
        );

        let mut invalid_revision = governance.clone().body;
        invalid_revision.lifecycle.governance_revision = 0;
        assert_eq!(
            PrivateSettlementPoolGovernanceV1::new(invalid_revision),
            Err(PrivateSettlementValidationError::InvalidPoolGovernanceLifecycle)
        );

        let mut invalid_interval = governance.clone().body;
        invalid_interval.lifecycle.retirement_height =
            Some(invalid_interval.lifecycle.activation_height);
        assert_eq!(
            PrivateSettlementPoolGovernanceV1::new(invalid_interval),
            Err(PrivateSettlementValidationError::InvalidPoolGovernanceLifecycle)
        );
    }

    #[test]
    fn auditor_approval_is_purpose_bound_and_signature_checked() {
        let (policy, signing_keys) = policy(DataSpaceId::new(1));
        let auditor = &policy.body.auditors[0];
        let signing = signing_keys
            .iter()
            .find(|key| key.public_key() == &auditor.signing_key)
            .expect("matching signing key");
        let body = PrivateSettlementAuditApprovalBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: network(1),
            bundle_id: hash(9),
            leg_ordinal: 0,
            dataspace_id: DataSpaceId::new(1),
            auditor_id: auditor.auditor_id.clone(),
            audit_policy_digest: policy.policy_digest,
            audit_key_epoch: policy.body.key_epoch,
            proof_digest: hash(10),
            capsule_digest: hash(11),
            delta_digest: hash(12),
            old_root: PrivacyRootV1::new([13; 32]),
            new_root: PrivacyRootV1::new([14; 32]),
            expiry_height: 100,
        };
        let approval = PrivateSettlementAuditApprovalV1 {
            signature: SignatureOf::try_new(signing.private_key(), &body)
                .expect("fixture approval signs"),
            body,
        };
        approval.verify(&policy, 20).expect("approval verifies");
        assert_eq!(
            format!("{policy:?}"),
            "PrivateSettlementAuditPolicyV1(<restricted>)"
        );
        assert_eq!(
            format!("{:?}", policy.body),
            "PrivateSettlementAuditPolicyBodyV1(<restricted>)"
        );
        assert_eq!(
            format!("{auditor:?}"),
            "PrivateSettlementAuditorV1(<restricted>)"
        );
        assert_eq!(
            format!("{:?}", approval.body),
            "PrivateSettlementAuditApprovalBodyV1(<restricted>)"
        );
        assert_eq!(
            format!("{approval:?}"),
            "PrivateSettlementAuditApprovalV1(<restricted>)"
        );

        let mut substituted = approval.clone();
        substituted.body.proof_digest = hash(99);
        assert_eq!(
            substituted.verify(&policy, 20),
            Err(PrivateSettlementValidationError::InvalidAuditSignature)
        );
    }

    #[test]
    fn phase_certificate_requires_three_of_four_signers() {
        let certificate = PrivateSettlementPhaseCertificateV1 {
            body: PrivateSettlementPhaseBodyV1 {
                network_id: network(1),
                bundle_id: hash(2),
                manifest_digest: hash(3),
                leg_ordinal: 0,
                route: route(1),
                delta_digest: hash(4),
                authority_digest: hash(5),
                prepared_bundle_digest: Hash::prehashed([0; Hash::LENGTH]),
                phase: PrivateSettlementPhaseV1::Prepare,
                authority_context_height: 10,
                expiry_height: 100,
            },
            authority_catalog_index: 0,
            signers_bitmap: 0b0111,
            aggregate_signature: vec![1; PRIVATE_SETTLEMENT_BLS_BYTES_V1],
        };
        certificate.validate_shape().expect("three signers qualify");
        let mut prepare_with_bundle_digest = certificate.clone();
        prepare_with_bundle_digest.body.prepared_bundle_digest = hash(6);
        assert_eq!(
            prepare_with_bundle_digest.validate_shape(),
            Err(PrivateSettlementValidationError::InvalidPhaseCertificate)
        );
        let mut commit_without_bundle_digest = certificate.clone();
        commit_without_bundle_digest.body.phase = PrivateSettlementPhaseV1::Commit;
        assert_eq!(
            commit_without_bundle_digest.validate_shape(),
            Err(PrivateSettlementValidationError::InvalidPhaseCertificate)
        );
        commit_without_bundle_digest.body.prepared_bundle_digest = hash(6);
        commit_without_bundle_digest
            .validate_shape()
            .expect("Commit requires the non-zero complete Prepare-barrier digest");

        let mut two = certificate;
        two.signers_bitmap = 0b0011;
        assert_eq!(
            two.validate_shape(),
            Err(PrivateSettlementValidationError::InvalidPhaseCertificate)
        );
        let mut four = two;
        four.signers_bitmap = 0b1111;
        assert_eq!(
            four.validate_shape(),
            Err(PrivateSettlementValidationError::InvalidPhaseCertificate)
        );
    }

    #[test]
    fn authority_catalog_deduplicates_rosters_and_reconstructs_route_bound_authorities() {
        let manifest = manifest(3);
        let (validators, validator_pops) = measured_validator_material();
        let authorities = manifest
            .legs
            .iter()
            .map(|leg| measured_authority(leg.route, &validators, &validator_pops))
            .collect::<Vec<_>>();
        let catalog =
            PrivateSettlementAuthorityCatalogV1::from_leg_authorities(&manifest, &authorities)
                .expect("shared roster compacts");

        assert_eq!(catalog.rosters.len(), 1);
        assert_eq!(catalog.leg_roster_indices, vec![0, 0, 0]);
        catalog
            .validate_for_manifest(&manifest)
            .expect("canonical catalog validates");
        for (index, expected) in authorities.iter().enumerate() {
            assert_eq!(
                catalog
                    .authority_for_leg(&manifest, index)
                    .expect("leg authority resolves"),
                *expected
            );
        }

        let encoded = norito::encode_canonical(&catalog).expect("catalog encodes");
        let decoded = norito::decode_canonical::<PrivateSettlementAuthorityCatalogV1>(&encoded)
            .expect("catalog decodes");
        assert_eq!(decoded, catalog);
    }

    #[test]
    fn authority_catalog_rejects_conflicts_and_noncanonical_references() {
        let manifest = manifest(2);
        let (validators, validator_pops) = measured_validator_material();
        let authorities = manifest
            .legs
            .iter()
            .map(|leg| measured_authority(leg.route, &validators, &validator_pops))
            .collect::<Vec<_>>();
        let catalog =
            PrivateSettlementAuthorityCatalogV1::from_leg_authorities(&manifest, &authorities)
                .expect("shared roster compacts");

        let mut conflicting = authorities.clone();
        conflicting[1].validator_pops[0][0] ^= 1;
        assert_eq!(
            PrivateSettlementAuthorityCatalogV1::from_leg_authorities(&manifest, &conflicting),
            Err(PrivateSettlementValidationError::InvalidAuthorityCatalog)
        );

        let mut duplicate_roster = catalog.clone();
        duplicate_roster.rosters.push(catalog.rosters[0].clone());
        duplicate_roster.leg_roster_indices = vec![0, 1];
        assert_eq!(
            duplicate_roster.validate_for_manifest(&manifest),
            Err(PrivateSettlementValidationError::InvalidAuthorityCatalog)
        );

        let mut noncanonical = catalog.clone();
        noncanonical
            .rosters
            .push(PrivateSettlementCommitteeRosterV1 {
                validator_set_hash: HashOf::new(&vec![
                    PeerId::from(
                        KeyPair::from_seed(vec![0xC1; 32], Algorithm::BlsNormal)
                            .public_key()
                            .clone(),
                    ),
                    PeerId::from(
                        KeyPair::from_seed(vec![0xC2; 32], Algorithm::BlsNormal)
                            .public_key()
                            .clone(),
                    ),
                    PeerId::from(
                        KeyPair::from_seed(vec![0xC3; 32], Algorithm::BlsNormal)
                            .public_key()
                            .clone(),
                    ),
                    PeerId::from(
                        KeyPair::from_seed(vec![0xC4; 32], Algorithm::BlsNormal)
                            .public_key()
                            .clone(),
                    ),
                ]),
                validators: vec![
                    PeerId::from(
                        KeyPair::from_seed(vec![0xC1; 32], Algorithm::BlsNormal)
                            .public_key()
                            .clone(),
                    ),
                    PeerId::from(
                        KeyPair::from_seed(vec![0xC2; 32], Algorithm::BlsNormal)
                            .public_key()
                            .clone(),
                    ),
                    PeerId::from(
                        KeyPair::from_seed(vec![0xC3; 32], Algorithm::BlsNormal)
                            .public_key()
                            .clone(),
                    ),
                    PeerId::from(
                        KeyPair::from_seed(vec![0xC4; 32], Algorithm::BlsNormal)
                            .public_key()
                            .clone(),
                    ),
                ],
                validator_pops: vec![vec![0xC1; PRIVATE_SETTLEMENT_BLS_BYTES_V1]; 4],
            });
        noncanonical.leg_roster_indices = vec![1, 0];
        assert_eq!(
            noncanonical.validate_for_manifest(&manifest),
            Err(PrivateSettlementValidationError::InvalidAuthorityCatalog)
        );

        let mut out_of_range = catalog;
        out_of_range.leg_roster_indices[1] = 1;
        assert_eq!(
            out_of_range.validate_for_manifest(&manifest),
            Err(PrivateSettlementValidationError::InvalidAuthorityCatalog)
        );
    }

    #[test]
    fn phase_vote_and_prepare_barrier_roundtrip_with_closed_shape() {
        let receipt = measured_receipt(2);
        let body = receipt.legs[0].prepare.body;
        let authority = receipt
            .authority_catalog
            .authority_for_leg(&receipt.manifest, 0)
            .expect("fixture authority resolves");
        let vote = PrivateSettlementPhaseVoteV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            body,
            signer: authority.validators[0].clone(),
            signature: vec![0xA5; PRIVATE_SETTLEMENT_BLS_BYTES_V1],
        };
        vote.validate_shape().expect("phase vote shape");
        let vote_bytes = norito::encode_canonical(&vote).expect("vote encodes");
        let decoded_vote: PrivateSettlementPhaseVoteV1 =
            norito::decode_canonical(&vote_bytes).expect("vote decodes");
        assert_eq!(decoded_vote, vote);

        let barrier = PrivateSettlementPrepareBarrierV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            manifest: receipt.manifest,
            authority_catalog: receipt.authority_catalog,
            deltas: receipt.legs.iter().map(|leg| leg.delta.clone()).collect(),
            prepare_certificates: receipt.legs.iter().map(|leg| leg.prepare.clone()).collect(),
            prepared_bundle_digest: hash(0xE5),
        };
        barrier.validate_shape().expect("barrier shape");
        let digest = barrier
            .computed_prepared_bundle_digest()
            .expect("barrier digest");
        let mut quorum_equivalent_encoding = barrier.clone();
        quorum_equivalent_encoding.prepare_certificates[0].signers_bitmap = 0b1011;
        quorum_equivalent_encoding.prepare_certificates[0].aggregate_signature =
            vec![0x5A; PRIVATE_SETTLEMENT_BLS_BYTES_V1];
        assert_eq!(
            quorum_equivalent_encoding
                .computed_prepared_bundle_digest()
                .expect("normalized barrier digest"),
            digest,
            "the digest binds the certified body, not its quorum encoding"
        );
        assert!(barrier.quorum_equivalent_to(&quorum_equivalent_encoding));
        let mut substituted_statement = quorum_equivalent_encoding.clone();
        substituted_statement.prepare_certificates[0]
            .body
            .delta_digest = hash(0x44);
        assert!(!barrier.quorum_equivalent_to(&substituted_statement));
        let json = norito::json::to_json(&barrier).expect("barrier JSON encodes");
        let decoded: PrivateSettlementPrepareBarrierV1 =
            norito::json::from_json(&json).expect("barrier JSON decodes");
        assert_eq!(decoded, barrier);

        let mut incomplete = barrier;
        incomplete.prepare_certificates.pop();
        assert_eq!(
            incomplete.validate_shape(),
            Err(PrivateSettlementValidationError::InvalidPrepareBarrier)
        );
    }

    #[test]
    fn prepare_and_receipt_shapes_reject_cross_leg_recipient_reuse() {
        let mut receipt = measured_receipt(2);
        let reused = receipt.legs[0].delta.encrypted_outputs[0].recipient;
        receipt.legs[1].delta.encrypted_outputs[0].recipient = reused;
        assert_eq!(
            receipt.validate_shape(),
            Err(PrivateSettlementValidationError::DuplicateStateItem)
        );

        let barrier = PrivateSettlementPrepareBarrierV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            manifest: receipt.manifest,
            authority_catalog: receipt.authority_catalog,
            deltas: receipt.legs.iter().map(|leg| leg.delta.clone()).collect(),
            prepare_certificates: receipt.legs.iter().map(|leg| leg.prepare.clone()).collect(),
            prepared_bundle_digest: hash(0xE5),
        };
        assert_eq!(
            barrier.validate_shape(),
            Err(PrivateSettlementValidationError::InvalidPrepareBarrier)
        );
    }

    #[test]
    fn sidecar_availability_signature_is_purpose_and_bundle_bound() {
        let manifest = manifest(2);
        let body = PrivateSettlementSidecarAvailabilityBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: manifest.network_id,
            bundle_id: manifest.bundle_id,
            leg_ordinal: 0,
            route: manifest.legs[0].route,
            authority_digest: hash(0xD1),
            authority_context_height: manifest.authority_context_height,
            payload_digest: manifest.legs[0].payload_digest,
            payload_bytes: 1,
            retention_until_height: manifest.expiry_height,
        };
        let certificate = PrivateSettlementSidecarAvailabilityV1 {
            body,
            signers_bitmap: 0b0111,
            aggregate_signature: vec![1; PRIVATE_SETTLEMENT_BLS_BYTES_V1],
        };
        certificate
            .validate_shape()
            .expect("availability certificate shape");
        let preimage = certificate
            .signature_preimage()
            .expect("availability preimage encodes");
        assert!(preimage.starts_with(SIDECAR_AVAILABILITY_SIGNATURE_DOMAIN_V1));
        let body_bytes = norito::encode_canonical(&certificate.body).expect("body encodes");
        let length_offset = SIDECAR_AVAILABILITY_SIGNATURE_DOMAIN_V1.len();
        assert_eq!(
            &preimage[length_offset..length_offset + std::mem::size_of::<u64>()],
            &u64::try_from(body_bytes.len())
                .expect("fixture body length fits u64")
                .to_le_bytes()
        );
        assert_eq!(
            &preimage[length_offset + std::mem::size_of::<u64>()..],
            body_bytes.as_slice()
        );

        let mut substituted = certificate.clone();
        substituted.body.bundle_id = hash(0xD2);
        assert_ne!(
            preimage,
            substituted
                .signature_preimage()
                .expect("substituted body encodes")
        );
        let mut missing = certificate;
        missing.body.bundle_id = Hash::prehashed([0; Hash::LENGTH]);
        assert_eq!(
            missing.validate_shape(),
            Err(PrivateSettlementValidationError::InvalidAvailabilityCertificate)
        );
    }

    #[test]
    fn auditor_view_attestation_is_purpose_height_lifecycle_and_responder_bound() {
        let (validators, validator_pops) = measured_validator_material();
        let authority = measured_authority(route(1), &validators, &validator_pops);
        let body = PrivateSettlementAuditorViewAttestationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: network(1),
            payload_digest: hash(0xD3),
            view_digest: hash(0xD4),
            authority_digest: authority.digest().expect("authority digest"),
            lifecycle_code: PRIVATE_SETTLEMENT_LIFECYCLE_COLLECTING_V1,
            authoritative_height: 19,
            responder: validators[0].clone(),
        };
        body.validate_shape().expect("attestation body shape");
        let preimage = body.signature_preimage().expect("attestation preimage");
        assert!(preimage.starts_with(AUDITOR_VIEW_ATTESTATION_SIGNATURE_DOMAIN_V1));

        let mut substituted = body.clone();
        substituted.authoritative_height += 1;
        assert_ne!(
            preimage,
            substituted
                .signature_preimage()
                .expect("height-substituted preimage")
        );
        substituted = body.clone();
        substituted.lifecycle_code = PRIVATE_SETTLEMENT_LIFECYCLE_AUDITED_V1;
        assert_ne!(
            preimage,
            substituted
                .signature_preimage()
                .expect("lifecycle-substituted preimage")
        );
        substituted = body.clone();
        substituted.responder = validators[1].clone();
        assert_ne!(
            preimage,
            substituted
                .signature_preimage()
                .expect("responder-substituted preimage")
        );

        let mut invalid = body;
        invalid.lifecycle_code = PRIVATE_SETTLEMENT_LIFECYCLE_EXPIRED_V1.saturating_add(1);
        assert_eq!(
            invalid.validate_shape(),
            Err(PrivateSettlementValidationError::InvalidAuditorViewAttestation)
        );
    }

    #[test]
    fn audit_approval_acknowledgement_attestation_binds_request_view_and_responder() {
        let (validators, validator_pops) = measured_validator_material();
        let authority = measured_authority(route(1), &validators, &validator_pops);
        let body = PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: network(1),
            payload_digest: hash(0xE3),
            approval_digest: hash(0xE4),
            acknowledgement_digest: hash(0xE5),
            authority_digest: authority.digest().expect("authority digest"),
            lifecycle_code: PRIVATE_SETTLEMENT_LIFECYCLE_AUDITED_V1,
            authoritative_height: 23,
            responder: validators[0].clone(),
        };
        body.validate_shape().expect("acknowledgement body shape");
        let preimage = body
            .signature_preimage()
            .expect("acknowledgement attestation preimage");
        assert!(
            preimage.starts_with(AUDIT_APPROVAL_ACKNOWLEDGEMENT_ATTESTATION_SIGNATURE_DOMAIN_V1)
        );

        let mut substituted = body.clone();
        substituted.approval_digest = hash(0xE6);
        assert_ne!(
            preimage,
            substituted
                .signature_preimage()
                .expect("approval-substituted preimage")
        );
        substituted = body.clone();
        substituted.acknowledgement_digest = hash(0xE7);
        assert_ne!(
            preimage,
            substituted
                .signature_preimage()
                .expect("acknowledgement-substituted preimage")
        );
        substituted = body.clone();
        substituted.responder = validators[1].clone();
        assert_ne!(
            preimage,
            substituted
                .signature_preimage()
                .expect("responder-substituted preimage")
        );

        let mut invalid = body;
        invalid.lifecycle_code = PRIVATE_SETTLEMENT_LIFECYCLE_PREPARED_V1;
        assert_eq!(
            invalid.validate_shape(),
            Err(PrivateSettlementValidationError::InvalidAuditApprovalAcknowledgementAttestation)
        );
    }

    #[test]
    fn fixed_output_codec_rejects_variable_or_unbound_ciphertext() {
        let profile = PrivateSettlementProofProfileV1::IvmPrivateNoteFixed2In3Out;
        let output_commitments = vec![
            PrivacyCommitmentV1::new([9; 32]),
            PrivacyCommitmentV1::new([10; 32]),
            PrivacyCommitmentV1::new([11; 32]),
        ];
        let mut ciphertext = vec![1; PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1];
        ciphertext[..4].copy_from_slice(b"IPNE");
        let encrypted_outputs = (0..PRIVATE_SETTLEMENT_OUTPUT_SLOTS_V1)
            .map(|index| PrivacyEncryptedOutputV1 {
                recipient: PrivacyRecipientIdV1::new([20 + u8::try_from(index).unwrap(); 32]),
                ephemeral_public_key: PrivacyEncryptionKeyV1::new(
                    [30 + u8::try_from(index).unwrap(); 32],
                ),
                commitment: output_commitments[index],
                ciphertext: ciphertext.clone(),
            })
            .collect::<Vec<_>>();
        let statement = PrivateSettlementProofStatementV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            profile,
            proof_profile_digest: profile.digest(),
            network_id: network(1),
            bundle_id: hash(2),
            leg_ordinal: 0,
            route: route(1),
            authority_context_height: 10,
            pool_id: PrivacyPoolIdV1::new([3; 32]),
            asset_binding_commitment: hash(4),
            old_root: PrivacyRootV1::new([5; 32]),
            new_root: PrivacyRootV1::new([6; 32]),
            old_epoch: 1,
            new_epoch: 2,
            nullifiers: vec![
                PrivacyNullifierV1::new([7; 32]),
                PrivacyNullifierV1::new([8; 32]),
            ],
            output_commitments,
            encrypted_outputs: encrypted_outputs.clone(),
            audit_plaintext_commitment: hash(16),
            audit_capsule_digest: hash(12),
            audit_policy_digest: hash(13),
            audit_key_epoch: 1,
            fee_intent_digest: hash(14),
            reimbursement_terms_commitment: hash(15),
            reimbursement_leg_ordinal: 0,
            expiry_height: 100,
        };
        statement.validate().expect("statement shape is valid");
        let statement_bytes = norito::encode_canonical(&statement).expect("statement encodes");
        let decoded_statement =
            norito::decode_canonical::<PrivateSettlementProofStatementV1>(&statement_bytes)
                .expect("statement decodes");
        assert_eq!(decoded_statement, statement);
        assert_eq!(decoded_statement.new_root, PrivacyRootV1::new([6; 32]));
        assert_eq!(decoded_statement.new_epoch, 2);
        let mut zero_successor = statement.clone();
        zero_successor.new_root = PrivacyRootV1::new([0; 32]);
        assert_eq!(
            zero_successor.validate(),
            Err(PrivateSettlementValidationError::ZeroCommitment)
        );
        let mut unchanged_successor = statement.clone();
        unchanged_successor.new_root = unchanged_successor.old_root;
        assert_eq!(
            unchanged_successor.validate(),
            Err(PrivateSettlementValidationError::InvalidEpoch)
        );
        let mut skipped_epoch = statement.clone();
        skipped_epoch.new_epoch = 3;
        assert_eq!(
            skipped_epoch.validate(),
            Err(PrivateSettlementValidationError::InvalidEpoch)
        );
        let mut delta = PrivateSettlementDeltaV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            bundle_id: statement.bundle_id,
            leg_ordinal: statement.leg_ordinal,
            route: statement.route,
            pool_id: statement.pool_id,
            asset_binding_commitment: statement.asset_binding_commitment,
            old_root: statement.old_root,
            new_root: statement.new_root,
            old_epoch: statement.old_epoch,
            new_epoch: statement.new_epoch,
            nullifiers: statement.nullifiers.clone(),
            output_commitments: statement.output_commitments.clone(),
            encrypted_outputs: encrypted_outputs.clone(),
            statement_digest: statement.digest().expect("statement hashes"),
            proof_digest: hash(16),
            capsule_digest: statement.audit_capsule_digest,
            audit_policy_digest: statement.audit_policy_digest,
            audit_key_epoch: statement.audit_key_epoch,
        };
        delta.validate_against(&statement).expect("delta aligns");
        let mut reused_statement_recipient = statement.clone();
        reused_statement_recipient.encrypted_outputs[1].recipient =
            reused_statement_recipient.encrypted_outputs[0].recipient;
        assert_eq!(
            reused_statement_recipient.validate(),
            Err(PrivateSettlementValidationError::DuplicateStateItem)
        );
        let mut reused_delta_recipient = delta.clone();
        reused_delta_recipient.encrypted_outputs[1].recipient =
            reused_delta_recipient.encrypted_outputs[0].recipient;
        assert_eq!(
            reused_delta_recipient.validate_public_shape(),
            Err(PrivateSettlementValidationError::DuplicateStateItem)
        );
        let mut substituted_successor = delta.clone();
        substituted_successor.new_root = PrivacyRootV1::new([17; 32]);
        assert_eq!(
            substituted_successor.validate_against(&statement),
            Err(PrivateSettlementValidationError::DeltaStatementMismatch)
        );
        let mut substituted_epoch = delta.clone();
        substituted_epoch.old_epoch = 2;
        substituted_epoch.new_epoch = 3;
        assert_eq!(
            substituted_epoch.validate_against(&statement),
            Err(PrivateSettlementValidationError::DeltaStatementMismatch)
        );
        delta.encrypted_outputs[2].ciphertext.pop();
        assert_eq!(
            delta.validate_against(&statement),
            Err(PrivateSettlementValidationError::InvalidEncryptedOutput { index: 2 })
        );
        let mut malformed_statement = statement;
        malformed_statement.encrypted_outputs[2].ciphertext.pop();
        assert_eq!(
            malformed_statement.validate(),
            Err(PrivateSettlementValidationError::InvalidEncryptedOutput { index: 2 })
        );
    }
}
