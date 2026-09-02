//! Public and restricted Torii DTOs for atomic private cross-dataspace settlement.
//!
//! Public status objects contain only protocol-allowlisted identifiers, routes,
//! timing, commitments, roots, nullifiers, ciphertexts, quorum certificates,
//! sponsor/fee terms, and terminal state.  Proof bytes and encrypted audit
//! capsules appear only in authenticated restricted requests or auditor
//! responses.  Auditor plaintext is never an HTTP DTO.

use core::fmt;

use iroha_crypto::{Algorithm, Hash, PublicKey, Signature};
use iroha_data_model::NetworkId;
use iroha_data_model::nexus::{
    ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, AtomicPrivateSettlementV1,
    PRIVATE_SETTLEMENT_LIFECYCLE_ABORTED_V1, PRIVATE_SETTLEMENT_LIFECYCLE_AUDITED_V1,
    PRIVATE_SETTLEMENT_LIFECYCLE_COLLECTING_V1, PRIVATE_SETTLEMENT_LIFECYCLE_COMMIT_CERTIFIED_V1,
    PRIVATE_SETTLEMENT_LIFECYCLE_EXPIRED_V1, PRIVATE_SETTLEMENT_LIFECYCLE_FINALIZED_V1,
    PRIVATE_SETTLEMENT_LIFECYCLE_PREPARED_V1, PrivateSettlementAbortReceiptV1,
    PrivateSettlementAuditApprovalAcknowledgementAttestationV1,
    PrivateSettlementAuditApprovalAcknowledgementDigestMaterialV1,
    PrivateSettlementAuditApprovalV1, PrivateSettlementAuditCapsuleV1,
    PrivateSettlementAuditPolicyV1, PrivateSettlementAuditorViewAttestationV1,
    PrivateSettlementAuditorViewDigestMaterialV1, PrivateSettlementAvailabilityShareV1,
    PrivateSettlementCommitteeAuthorityV1, PrivateSettlementDeltaV1, PrivateSettlementLegPayloadV1,
    PrivateSettlementPhaseCertificateV1, PrivateSettlementPhaseV1, PrivateSettlementPhaseVoteV1,
    PrivateSettlementPrepareBarrierV1, PrivateSettlementProofStatementV1,
    PrivateSettlementProvisionalLegMaterialV1, PrivateSettlementReceiptV1,
    PrivateSettlementRouteV1, PrivateSettlementSidecarAvailabilityV1,
    private_settlement_proof_digest_v1,
};
use iroha_data_model::transaction::SignedTransaction;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Durable first-release settlement lifecycle projected by Torii.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(tag = "status", content = "value", rename_all = "snake_case")]
pub enum PrivateSettlementLifecycleDtoV1 {
    /// Encrypted sidecar is durable and auditor approvals are being collected.
    Collecting,
    /// The governed local auditor threshold is durable.
    Audited,
    /// The committee durably staged the verified private-state delta.
    Prepared,
    /// The local Commit QC is durable.
    CommitCertified,
    /// Every leg was atomically applied and a public receipt is durable.
    Finalized,
    /// An authoritative abort released the staged lock.
    Aborted,
    /// Height expiry released the staged lock.
    Expired,
}

impl PrivateSettlementLifecycleDtoV1 {
    /// Return the stable lifecycle code committed by node view attestations.
    #[must_use]
    pub const fn attestation_code(self) -> u8 {
        match self {
            Self::Collecting => PRIVATE_SETTLEMENT_LIFECYCLE_COLLECTING_V1,
            Self::Audited => PRIVATE_SETTLEMENT_LIFECYCLE_AUDITED_V1,
            Self::Prepared => PRIVATE_SETTLEMENT_LIFECYCLE_PREPARED_V1,
            Self::CommitCertified => PRIVATE_SETTLEMENT_LIFECYCLE_COMMIT_CERTIFIED_V1,
            Self::Finalized => PRIVATE_SETTLEMENT_LIFECYCLE_FINALIZED_V1,
            Self::Aborted => PRIVATE_SETTLEMENT_LIFECYCLE_ABORTED_V1,
            Self::Expired => PRIVATE_SETTLEMENT_LIFECYCLE_EXPIRED_V1,
        }
    }
}

/// Account-authenticated upload of one complete encrypted settlement leg.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementLegUploadRequestV1 {
    /// Exact public atomic bundle manifest.
    pub manifest: AtomicPrivateSettlementV1,
    /// Governed local auditor policy active at the authority context.
    pub audit_policy: PrivateSettlementAuditPolicyV1,
    /// Exact four-validator participant authority.
    pub committee_authority: PrivateSettlementCommitteeAuthorityV1,
    /// Proof, fixed opaque delta, encrypted capsule, and availability certificate.
    pub payload: PrivateSettlementLegPayloadV1,
}

/// Sponsor-authenticated request for one node's provisional availability share.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAvailabilityShareRequestV1 {
    /// Exact encrypted material that the node must persist before signing.
    pub material: PrivateSettlementProvisionalLegMaterialV1,
}

/// Redacted response carrying one node-authenticated availability share.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAvailabilityShareResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the exact encrypted leg material.
    pub payload_digest: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Whether this request stored new bytes or reused an exact durable record.
    pub disposition: PrivateSettlementLegUploadDispositionV1,
    /// Exact BLS-normal share issued by this node after persistence.
    pub share: PrivateSettlementAvailabilityShareV1,
}

/// Sponsor-authenticated request for one independently verified Prepare vote.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementPrepareVoteRequestV1 {
    /// Exact finalized public manifest retained with the local encrypted leg.
    pub manifest: AtomicPrivateSettlementV1,
    /// Content address selecting the exact local participant leg.
    pub payload_digest: Hash,
}

/// Sponsor-authenticated request for one exact complete-barrier Commit vote.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementCommitVoteRequestV1 {
    /// Content address selecting the exact locally staged participant leg.
    pub payload_digest: Hash,
    /// Exact complete all-Prepare barrier to bind into Commit.
    pub barrier: PrivateSettlementPrepareBarrierV1,
}

/// Redacted response carrying one node-authenticated participant phase vote.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementPhaseVoteResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the exact local encrypted leg.
    pub payload_digest: Hash,
    /// Canonical participant leg ordinal.
    pub leg_ordinal: u8,
    /// Exact BLS-normal phase vote issued by this node.
    pub vote: PrivateSettlementPhaseVoteV1,
}

/// Sponsor-authenticated handoff of one aggregate phase certificate to signers.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementPhaseCertificateRequestV1 {
    /// Exact finalized public manifest retained with the local encrypted leg.
    pub manifest: AtomicPrivateSettlementV1,
    /// Content address selecting the exact local participant leg.
    pub payload_digest: Hash,
    /// Exact three-of-four aggregate Prepare or Commit certificate.
    pub certificate: PrivateSettlementPhaseCertificateV1,
}

/// Redacted acknowledgement that one aggregate phase certificate is durable.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementPhaseCertificateResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the exact local encrypted leg.
    pub payload_digest: Hash,
    /// Canonical participant leg ordinal.
    pub leg_ordinal: u8,
    /// Durable participant phase.
    pub phase: PrivateSettlementPhaseV1,
    /// Current durable local lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
}

/// Sponsor-only recovery view for exact locally durable phase certificates.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementPhaseCertificatesResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the exact local encrypted leg.
    pub payload_digest: Hash,
    /// Canonical participant leg ordinal.
    pub leg_ordinal: u8,
    /// Current durable local lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
    /// Exact locally durable Prepare QC, or explicit absence.
    #[norito(required)]
    pub prepare_certificate: Option<PrivateSettlementPhaseCertificateV1>,
    /// Exact locally durable Commit QC, or explicit absence.
    #[norito(required)]
    pub commit_certificate: Option<PrivateSettlementPhaseCertificateV1>,
}

/// Idempotent encrypted-leg upload disposition.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(tag = "result", content = "value", rename_all = "snake_case")]
pub enum PrivateSettlementLegUploadDispositionV1 {
    /// New encrypted bytes became durable.
    Stored,
    /// The exact canonical encrypted bytes were already durable.
    AlreadyStored,
}

/// Redacted response for an encrypted-leg upload.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementLegUploadResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the exact encrypted upload.
    pub payload_digest: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Idempotent storage outcome.
    pub disposition: PrivateSettlementLegUploadDispositionV1,
    /// Current durable lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
}

/// Redacted lifecycle response for one encrypted settlement leg.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementLegStatusResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the encrypted upload.
    pub payload_digest: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Public participant route.
    pub route: PrivateSettlementRouteV1,
    /// Height at which encrypted bytes first became durable.
    pub stored_at_height: u64,
    /// Latest durable lifecycle height.
    pub lifecycle_height: u64,
    /// Final valid global height.
    pub expiry_height: u64,
    /// Current durable lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
}

/// Restricted proof view returned only to an exact participant validator.
///
/// This DTO deliberately omits encrypted capsule bytes and every plaintext
/// audit opening. Validators receive only the proof, fixed opaque delta,
/// governed approval material, and durable-availability evidence needed for
/// Prepare.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementCommitteeProofResponseV1 {
    /// Exact public bundle manifest.
    pub manifest: AtomicPrivateSettlementV1,
    /// Governed auditor policy used to verify signatures and threshold.
    pub audit_policy: PrivateSettlementAuditPolicyV1,
    /// Exact four-validator participant authority.
    pub committee_authority: PrivateSettlementCommitteeAuthorityV1,
    /// Restricted public proof statement.
    pub statement: PrivateSettlementProofStatementV1,
    /// Native fixed-profile zero-knowledge proof bytes.
    pub proof: Vec<u8>,
    /// Opaque fixed-shape private-state delta.
    pub delta: PrivateSettlementDeltaV1,
    /// Canonical governed auditor approvals collected so far.
    pub audit_approvals: Vec<PrivateSettlementAuditApprovalV1>,
    /// Digest of the encrypted capsule; the capsule itself is absent.
    pub audit_capsule_digest: Hash,
    /// Durable restricted-DA certificate.
    pub availability: PrivateSettlementSidecarAvailabilityV1,
    /// Current durable lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
}

impl fmt::Debug for PrivateSettlementCommitteeProofResponseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateSettlementCommitteeProofResponseV1")
            .field("bundle_id", &self.manifest.bundle_id)
            .field("leg_ordinal", &self.statement.leg_ordinal)
            .field("route", &self.statement.route)
            .field("audit_approvals", &self.audit_approvals.len())
            .field("lifecycle", &self.lifecycle)
            .finish_non_exhaustive()
    }
}

/// Governed-auditor request for one restricted encrypted capsule view.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAuditorCapsuleRequestV1 {
    /// Exact current governed policy under which the request is authorized.
    pub audit_policy: PrivateSettlementAuditPolicyV1,
}

/// Restricted capsule response returned only to a governed local auditor.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAuditorCapsuleResponseV1 {
    /// Node-authoritative height used for access and lifecycle evaluation.
    pub authoritative_height: u64,
    /// Exact public manifest used to recompute bindings.
    pub manifest: AtomicPrivateSettlementV1,
    /// Exact historical governed policy bound by the encrypted sidecar.
    pub audit_policy: PrivateSettlementAuditPolicyV1,
    /// Exact current governed policy used to authorize this restricted read.
    pub access_audit_policy: PrivateSettlementAuditPolicyV1,
    /// Exact consensus authority used to enforce key separation.
    pub committee_authority: PrivateSettlementCommitteeAuthorityV1,
    /// Restricted proof statement; proof bytes are deliberately absent.
    pub statement: PrivateSettlementProofStatementV1,
    /// Opaque fixed-shape private-state delta.
    pub delta: PrivateSettlementDeltaV1,
    /// Padded hybrid-encrypted capsule.
    pub audit_capsule: PrivateSettlementAuditCapsuleV1,
    /// Durable restricted-DA certificate.
    pub availability: PrivateSettlementSidecarAvailabilityV1,
    /// Current durable lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
    /// Purpose-separated BLS authentication by the exact responding validator.
    pub responder_attestation: PrivateSettlementAuditorViewAttestationV1,
}

impl PrivateSettlementAuditorCapsuleResponseV1 {
    /// Reconstruct the exact typed material committed by the responder.
    #[must_use]
    pub fn view_digest_material(&self) -> PrivateSettlementAuditorViewDigestMaterialV1 {
        PrivateSettlementAuditorViewDigestMaterialV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            authoritative_height: self.authoritative_height,
            manifest: self.manifest.clone(),
            audit_policy: self.audit_policy.clone(),
            access_audit_policy: self.access_audit_policy.clone(),
            committee_authority: self.committee_authority.clone(),
            statement: self.statement.clone(),
            delta: self.delta.clone(),
            audit_capsule: self.audit_capsule.clone(),
            availability: self.availability.clone(),
            lifecycle_code: self.lifecycle.attestation_code(),
        }
    }

    /// Compute the exact purpose-separated digest authenticated by the node.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the typed restricted view cannot be encoded.
    pub fn view_digest(&self) -> Result<Hash, norito::Error> {
        self.view_digest_material().digest()
    }
}

/// Governed-auditor submission of one purpose-separated approval.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAuditApprovalRequestV1 {
    /// Exact current governed policy under which the approval is submitted.
    pub audit_policy: PrivateSettlementAuditPolicyV1,
    /// Signed approval whose auditor identity must match the request key.
    pub approval: PrivateSettlementAuditApprovalV1,
}

/// Redacted result of durably collecting one approval.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAuditApprovalResponseV1 {
    /// Node-authoritative height at which the approval result is durable.
    pub authoritative_height: u64,
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the encrypted leg.
    pub payload_digest: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Exact four-validator participant authority serving the acknowledgement.
    pub committee_authority: PrivateSettlementCommitteeAuthorityV1,
    /// Number of distinct governed approvals now durable.
    pub collected: u8,
    /// Governed approval threshold.
    pub required: u8,
    /// Whether this request inserted new durable approval material.
    pub newly_recorded: bool,
    /// Current durable lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
    /// Purpose-separated BLS authentication by the exact responding validator.
    pub responder_attestation: PrivateSettlementAuditApprovalAcknowledgementAttestationV1,
}

impl PrivateSettlementAuditApprovalResponseV1 {
    /// Reconstruct the exact typed material committed by the responder.
    #[must_use]
    pub fn acknowledgement_digest_material(
        &self,
    ) -> PrivateSettlementAuditApprovalAcknowledgementDigestMaterialV1 {
        PrivateSettlementAuditApprovalAcknowledgementDigestMaterialV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            authoritative_height: self.authoritative_height,
            bundle_id: self.bundle_id,
            payload_digest: self.payload_digest,
            leg_ordinal: self.leg_ordinal,
            committee_authority: self.committee_authority.clone(),
            collected: self.collected,
            required: self.required,
            newly_recorded: self.newly_recorded,
            lifecycle_code: self.lifecycle.attestation_code(),
        }
    }

    /// Compute the exact purpose-separated acknowledgement digest.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if the typed response cannot be encoded.
    pub fn acknowledgement_digest(&self) -> Result<Hash, norito::Error> {
        self.acknowledgement_digest_material().digest()
    }
}

/// Redacted failure returned by shared private-settlement response validation.
///
/// The variants identify only the public validation boundary that rejected a
/// response. They deliberately retain no proof bytes, capsule material,
/// auditor identity, cryptographic key, digest, or underlying parser error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrivateSettlementResponseValidationErrorV1 {
    /// A committee proof view was malformed, inconsistent, or substituted.
    InvalidCommitteeProofResponse,
    /// An auditor capsule view or its responder attestation was invalid.
    InvalidAuditorCapsuleResponse,
    /// The supplied signing key was not governed or reused a committee key.
    InvalidAuditorKeySeparation,
    /// An audit-approval acknowledgement was invalid or request-substituted.
    InvalidAuditApprovalAcknowledgement,
}

impl fmt::Display for PrivateSettlementResponseValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidCommitteeProofResponse => {
                "private-settlement committee response validation failed"
            }
            Self::InvalidAuditorCapsuleResponse => {
                "private-settlement auditor response validation failed"
            }
            Self::InvalidAuditorKeySeparation => "private-settlement auditor key validation failed",
            Self::InvalidAuditApprovalAcknowledgement => {
                "private-settlement approval acknowledgement validation failed"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for PrivateSettlementResponseValidationErrorV1 {}

fn validate_response_availability_certificate_v1(
    certificate: &PrivateSettlementSidecarAvailabilityV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
) -> Result<(), PrivateSettlementResponseValidationErrorV1> {
    let invalid = PrivateSettlementResponseValidationErrorV1::InvalidCommitteeProofResponse;
    certificate.validate_shape().map_err(|_| invalid)?;
    authority.validate().map_err(|_| invalid)?;
    let authority_digest = authority.digest().map_err(|_| invalid)?;
    if certificate.body.route != authority.route
        || certificate.body.authority_digest != authority_digest
    {
        return Err(invalid);
    }
    let mut signer_keys = Vec::with_capacity(3);
    let mut signer_pops = Vec::with_capacity(3);
    for (index, (validator, pop)) in authority
        .validators
        .iter()
        .zip(&authority.validator_pops)
        .enumerate()
    {
        if validator.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
            || iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err()
        {
            return Err(invalid);
        }
        if certificate.signers_bitmap & (1_u8 << index) != 0 {
            signer_keys.push(validator.public_key());
            signer_pops.push(pop.as_slice());
        }
    }
    let preimage = certificate.signature_preimage().map_err(|_| invalid)?;
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &certificate.aggregate_signature,
        &signer_keys,
        &signer_pops,
    )
    .map_err(|_| invalid)
}

/// Validate a committee-only proof response against the exact requested leg.
///
/// This verifies the manifest, policy, authority and every validator proof of
/// possession, proof statement, fixed-shape delta, availability certificate,
/// proof/capsule/delta digests, and the canonical governed approval threshold.
/// No private response material is retained in an error.
///
/// # Errors
///
/// Returns [`PrivateSettlementResponseValidationErrorV1::InvalidCommitteeProofResponse`]
/// for any malformed, inconsistent, unauthenticated, stale, or substituted
/// field.
pub fn validate_private_settlement_committee_proof_response_v1(
    expected_network: &NetworkId,
    requested_payload_digest: Hash,
    response: &PrivateSettlementCommitteeProofResponseV1,
) -> Result<(), PrivateSettlementResponseValidationErrorV1> {
    let invalid = PrivateSettlementResponseValidationErrorV1::InvalidCommitteeProofResponse;
    response.manifest.validate().map_err(|_| invalid)?;
    response.audit_policy.validate().map_err(|_| invalid)?;
    response
        .committee_authority
        .validate()
        .map_err(|_| invalid)?;
    response.statement.validate().map_err(|_| invalid)?;
    response
        .delta
        .validate_against(&response.statement)
        .map_err(|_| invalid)?;
    validate_response_availability_certificate_v1(
        &response.availability,
        &response.committee_authority,
    )?;
    let statement_digest = response.statement.digest().map_err(|_| invalid)?;
    let proof_digest = private_settlement_proof_digest_v1(&response.proof);
    let delta_digest = response.delta.digest().map_err(|_| invalid)?;
    let authority_digest = response.committee_authority.digest().map_err(|_| invalid)?;
    let availability_digest = response.availability.digest().map_err(|_| invalid)?;
    let ordinal = usize::from(response.statement.leg_ordinal);
    let leg = response.manifest.legs.get(ordinal).ok_or(invalid)?;
    let availability = &response.availability.body;
    if &response.manifest.network_id != expected_network
        || response.statement.network_id != response.manifest.network_id
        || response.statement.authority_context_height != response.manifest.authority_context_height
        || response.statement.fee_intent_digest != response.manifest.fee_intent_digest
        || response.statement.reimbursement_terms_commitment
            != response.manifest.reimbursement_terms_commitment
        || response.statement.reimbursement_leg_ordinal
            != response.manifest.reimbursement_leg_ordinal
        || response.statement.expiry_height != response.manifest.expiry_height
        || availability.payload_digest != requested_payload_digest
        || leg.payload_digest != requested_payload_digest
        || leg.ordinal != response.statement.leg_ordinal
        || leg.delta_digest != delta_digest
        || leg.availability_certificate_digest != availability_digest
        || leg.route != response.statement.route
        || leg.pool_id != response.statement.pool_id
        || leg.asset_binding_commitment != response.statement.asset_binding_commitment
        || leg.audit_policy_digest != response.audit_policy.policy_digest
        || response.audit_policy.body.dataspace_id != response.statement.route.dataspace_id
        || response.statement.audit_policy_digest != response.audit_policy.policy_digest
        || response.statement.audit_key_epoch != response.audit_policy.body.key_epoch
        || response.committee_authority.route != response.statement.route
        || availability.network_id != response.manifest.network_id
        || availability.bundle_id != response.manifest.bundle_id
        || availability.leg_ordinal != response.statement.leg_ordinal
        || availability.route != response.statement.route
        || availability.authority_digest != authority_digest
        || availability.authority_context_height != response.manifest.authority_context_height
        || availability.retention_until_height < response.manifest.expiry_height
        || response.manifest.bundle_id != response.statement.bundle_id
        || response.manifest.bundle_id != response.delta.bundle_id
        || response.statement.leg_ordinal != response.delta.leg_ordinal
        || response.statement.route != response.delta.route
        || response.statement.pool_id != response.delta.pool_id
        || response.statement.asset_binding_commitment != response.delta.asset_binding_commitment
        || response.statement.audit_policy_digest != response.delta.audit_policy_digest
        || response.statement.audit_key_epoch != response.delta.audit_key_epoch
        || response.statement.audit_capsule_digest != response.audit_capsule_digest
        || response.delta.capsule_digest != response.audit_capsule_digest
        || response.delta.statement_digest != statement_digest
        || response.delta.proof_digest != proof_digest
    {
        return Err(invalid);
    }
    if response.audit_approvals.len() < usize::from(response.audit_policy.body.min_approvals) {
        return Err(invalid);
    }
    let mut previous_auditor = None;
    for approval in &response.audit_approvals {
        approval
            .verify(
                &response.audit_policy,
                response.manifest.authority_context_height,
            )
            .map_err(|_| invalid)?;
        if previous_auditor
            .as_ref()
            .is_some_and(|previous| previous >= &approval.body.auditor_id)
            || approval.body.network_id != response.statement.network_id
            || approval.body.bundle_id != response.statement.bundle_id
            || approval.body.leg_ordinal != response.statement.leg_ordinal
            || approval.body.dataspace_id != response.statement.route.dataspace_id
            || approval.body.audit_policy_digest != response.audit_policy.policy_digest
            || approval.body.audit_key_epoch != response.audit_policy.body.key_epoch
            || approval.body.proof_digest != proof_digest
            || approval.body.capsule_digest != response.audit_capsule_digest
            || approval.body.delta_digest != delta_digest
            || approval.body.old_root != response.delta.old_root
            || approval.body.new_root != response.delta.new_root
            || approval.body.expiry_height != response.statement.expiry_height
        {
            return Err(invalid);
        }
        previous_auditor = Some(approval.body.auditor_id.clone());
    }
    Ok(())
}

fn validate_private_settlement_auditor_view_attestation_v1(
    requested_payload_digest: Hash,
    response: &PrivateSettlementAuditorCapsuleResponseV1,
) -> Result<usize, PrivateSettlementResponseValidationErrorV1> {
    let invalid = PrivateSettlementResponseValidationErrorV1::InvalidAuditorCapsuleResponse;
    let attestation = &response.responder_attestation;
    attestation.validate_shape().map_err(|_| invalid)?;
    let authority_digest = response.committee_authority.digest().map_err(|_| invalid)?;
    let view_digest = response.view_digest().map_err(|_| invalid)?;
    let expected_body = iroha_data_model::nexus::PrivateSettlementAuditorViewAttestationBodyV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id: response.manifest.network_id,
        payload_digest: requested_payload_digest,
        view_digest,
        authority_digest,
        lifecycle_code: response.lifecycle.attestation_code(),
        authoritative_height: response.authoritative_height,
        responder: attestation.body.responder.clone(),
    };
    if attestation.body != expected_body {
        return Err(invalid);
    }
    let responder_index = response
        .committee_authority
        .validators
        .iter()
        .position(|validator| validator == &attestation.body.responder)
        .ok_or(invalid)?;
    let responder_pop = response
        .committee_authority
        .validator_pops
        .get(responder_index)
        .ok_or(invalid)?;
    if attestation.body.responder.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
        || iroha_crypto::bls_normal_pop_verify(
            attestation.body.responder.public_key(),
            responder_pop,
        )
        .is_err()
    {
        return Err(invalid);
    }
    let signature = Signature::try_from_bytes(&attestation.signature).map_err(|_| invalid)?;
    signature
        .verify(
            attestation.body.responder.public_key(),
            &attestation.body.signature_preimage().map_err(|_| invalid)?,
        )
        .map_err(|_| invalid)?;
    Ok(responder_index)
}

fn private_settlement_auditor_policy_access_is_valid_v1(
    historical_policy: &PrivateSettlementAuditPolicyV1,
    access_policy: &PrivateSettlementAuditPolicyV1,
    authority_context_height: u64,
    authoritative_height: u64,
) -> bool {
    if historical_policy.validate().is_err()
        || access_policy.validate().is_err()
        || !historical_policy.is_active_at(authority_context_height)
        || !access_policy.is_active_at(authoritative_height)
        || historical_policy.body.dataspace_id != access_policy.body.dataspace_id
    {
        return false;
    }
    if historical_policy == access_policy {
        return true;
    }
    // Policy activation intervals may overlap or be preactivated. The exact
    // governance revision is selected by WSV in core and attested by the
    // committee response; this verifier therefore checks monotonic restricted
    // policy lineage without inventing a second timestamp-based switch rule.
    historical_policy.body.policy_id == access_policy.body.policy_id
        && access_policy.body.revision > historical_policy.body.revision
        && access_policy.body.key_epoch > historical_policy.body.key_epoch
}

fn private_settlement_lifecycle_is_terminal_v1(lifecycle: PrivateSettlementLifecycleDtoV1) -> bool {
    matches!(
        lifecycle,
        PrivateSettlementLifecycleDtoV1::Finalized
            | PrivateSettlementLifecycleDtoV1::Aborted
            | PrivateSettlementLifecycleDtoV1::Expired
    )
}

/// Validate an auditor-only capsule response and authenticate its responder.
///
/// In addition to the public manifest, policy, authority, proof-statement,
/// delta, capsule, availability, and lifecycle bindings, this recomputes the
/// complete typed view digest and verifies the exact responder's BLS-normal
/// proof of possession and signature.
///
/// # Errors
///
/// Returns [`PrivateSettlementResponseValidationErrorV1::InvalidAuditorCapsuleResponse`]
/// for any malformed, inconsistent, unauthenticated, stale, or substituted
/// field.
pub fn validate_private_settlement_auditor_capsule_response_v1(
    expected_network: &NetworkId,
    requested_payload_digest: Hash,
    request: &PrivateSettlementAuditorCapsuleRequestV1,
    response: &PrivateSettlementAuditorCapsuleResponseV1,
) -> Result<usize, PrivateSettlementResponseValidationErrorV1> {
    let invalid = PrivateSettlementResponseValidationErrorV1::InvalidAuditorCapsuleResponse;
    response.manifest.validate().map_err(|_| invalid)?;
    response.audit_policy.validate().map_err(|_| invalid)?;
    response
        .access_audit_policy
        .validate()
        .map_err(|_| invalid)?;
    response
        .committee_authority
        .validate()
        .map_err(|_| invalid)?;
    response.statement.validate().map_err(|_| invalid)?;
    response
        .delta
        .validate_against(&response.statement)
        .map_err(|_| invalid)?;
    response
        .audit_capsule
        .validate_against(&response.audit_policy)
        .map_err(|_| invalid)?;
    validate_response_availability_certificate_v1(
        &response.availability,
        &response.committee_authority,
    )
    .map_err(|_| invalid)?;
    let capsule_digest = response.audit_capsule.digest().map_err(|_| invalid)?;
    let delta_digest = response.delta.digest().map_err(|_| invalid)?;
    let authority_digest = response.committee_authority.digest().map_err(|_| invalid)?;
    let availability_digest = response.availability.digest().map_err(|_| invalid)?;
    let ordinal = usize::from(response.statement.leg_ordinal);
    let leg = response.manifest.legs.get(ordinal).ok_or(invalid)?;
    let availability = &response.availability.body;
    let capsule_aad = &response.audit_capsule.aad;
    let lifecycle_height_is_valid =
        if private_settlement_lifecycle_is_terminal_v1(response.lifecycle) {
            response.authoritative_height <= availability.retention_until_height
        } else {
            response.authoritative_height <= response.manifest.expiry_height
        };
    if &response.manifest.network_id != expected_network
        || response.authoritative_height == 0
        || response.authoritative_height < response.manifest.authority_context_height
        || !lifecycle_height_is_valid
        || response.access_audit_policy != request.audit_policy
        || !private_settlement_auditor_policy_access_is_valid_v1(
            &response.audit_policy,
            &response.access_audit_policy,
            response.manifest.authority_context_height,
            response.authoritative_height,
        )
        || availability.payload_digest != requested_payload_digest
        || leg.payload_digest != requested_payload_digest
        || leg.ordinal != response.statement.leg_ordinal
        || leg.delta_digest != delta_digest
        || leg.availability_certificate_digest != availability_digest
        || leg.route != response.statement.route
        || leg.pool_id != response.statement.pool_id
        || leg.asset_binding_commitment != response.statement.asset_binding_commitment
        || leg.audit_policy_digest != response.audit_policy.policy_digest
        || response.audit_policy.body.dataspace_id != response.statement.route.dataspace_id
        || response.access_audit_policy.body.dataspace_id != response.statement.route.dataspace_id
        || response.committee_authority.route != response.statement.route
        || availability.network_id != response.manifest.network_id
        || availability.bundle_id != response.manifest.bundle_id
        || availability.leg_ordinal != response.statement.leg_ordinal
        || availability.route != response.statement.route
        || availability.authority_digest != authority_digest
        || availability.authority_context_height != response.manifest.authority_context_height
        || availability.retention_until_height < response.manifest.expiry_height
        || response.statement.network_id != response.manifest.network_id
        || response.manifest.bundle_id != response.statement.bundle_id
        || response.manifest.bundle_id != response.delta.bundle_id
        || response.statement.authority_context_height != response.manifest.authority_context_height
        || response.statement.leg_ordinal != response.delta.leg_ordinal
        || response.statement.route != response.delta.route
        || response.statement.pool_id != response.delta.pool_id
        || response.statement.asset_binding_commitment != response.delta.asset_binding_commitment
        || response.statement.audit_policy_digest != response.delta.audit_policy_digest
        || response.statement.audit_policy_digest != response.audit_policy.policy_digest
        || response.statement.audit_key_epoch != response.delta.audit_key_epoch
        || response.statement.audit_key_epoch != response.audit_policy.body.key_epoch
        || response.statement.fee_intent_digest != response.manifest.fee_intent_digest
        || response.statement.reimbursement_terms_commitment
            != response.manifest.reimbursement_terms_commitment
        || response.statement.reimbursement_leg_ordinal
            != response.manifest.reimbursement_leg_ordinal
        || response.statement.expiry_height != response.manifest.expiry_height
        || capsule_aad.network_id != response.statement.network_id
        || capsule_aad.bundle_id != response.statement.bundle_id
        || capsule_aad.leg_ordinal != response.statement.leg_ordinal
        || capsule_aad.route != response.statement.route
        || capsule_aad.authority_digest != authority_digest
        || capsule_aad.authority_context_height != response.statement.authority_context_height
        || capsule_aad.plaintext_commitment != response.statement.audit_plaintext_commitment
        || response.statement.audit_capsule_digest != capsule_digest
        || response.delta.capsule_digest != capsule_digest
    {
        return Err(invalid);
    }
    validate_private_settlement_auditor_view_attestation_v1(requested_payload_digest, response)
}

/// Validate that an auditor signing key is governed and consensus-separated.
///
/// The key must map to an auditor in the response's access policy. The same
/// stable auditor identity must occur in the historical capsule policy and its
/// wrapped-DEK roster, and neither relevant signing key may be reused by a
/// validator in the response's committee authority.
///
/// # Errors
///
/// Returns [`PrivateSettlementResponseValidationErrorV1::InvalidAuditorKeySeparation`]
/// when the key is not governed or is reused as a consensus key.
pub fn validate_private_settlement_auditor_identity_v1(
    auditor_signing_key: &PublicKey,
    response: &PrivateSettlementAuditorCapsuleResponseV1,
) -> Result<(), PrivateSettlementResponseValidationErrorV1> {
    let invalid = PrivateSettlementResponseValidationErrorV1::InvalidAuditorKeySeparation;
    response.audit_policy.validate().map_err(|_| invalid)?;
    response
        .access_audit_policy
        .validate()
        .map_err(|_| invalid)?;
    response
        .committee_authority
        .validate()
        .map_err(|_| invalid)?;
    for (validator, pop) in response
        .committee_authority
        .validators
        .iter()
        .zip(&response.committee_authority.validator_pops)
    {
        if validator.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
            || iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err()
        {
            return Err(invalid);
        }
    }
    if !private_settlement_auditor_policy_access_is_valid_v1(
        &response.audit_policy,
        &response.access_audit_policy,
        response.manifest.authority_context_height,
        response.authoritative_height,
    ) {
        return Err(invalid);
    }
    let access_auditor = response
        .access_audit_policy
        .body
        .auditors
        .iter()
        .find(|auditor| &auditor.signing_key == auditor_signing_key)
        .ok_or(invalid)?;
    let historical_auditor = response
        .audit_policy
        .body
        .auditors
        .iter()
        .find(|auditor| auditor.auditor_id == access_auditor.auditor_id)
        .ok_or(invalid)?;
    if !response
        .audit_capsule
        .wrapped_deks
        .iter()
        .any(|wrapped| wrapped.auditor_id == access_auditor.auditor_id)
    {
        return Err(invalid);
    }
    let committee_key_reused = response
        .committee_authority
        .validators
        .iter()
        .any(|validator| {
            validator.public_key() == auditor_signing_key
                || validator.public_key() == &historical_auditor.signing_key
        });
    if committee_key_reused {
        return Err(invalid);
    }
    Ok(())
}

/// Validate an audit-approval acknowledgement against the exact request.
///
/// This checks lifecycle and height bounds, request identifiers, governed
/// route, committee roster and proofs of possession, all acknowledgement
/// digests, and the exact responding validator's BLS-normal signature.
///
/// # Errors
///
/// Returns [`PrivateSettlementResponseValidationErrorV1::InvalidAuditApprovalAcknowledgement`]
/// for any malformed, inconsistent, unauthenticated, expired, or substituted
/// field.
pub fn validate_private_settlement_audit_approval_response_v1(
    requested_payload_digest: Hash,
    request: &PrivateSettlementAuditApprovalRequestV1,
    response: &PrivateSettlementAuditApprovalResponseV1,
) -> Result<usize, PrivateSettlementResponseValidationErrorV1> {
    let invalid = PrivateSettlementResponseValidationErrorV1::InvalidAuditApprovalAcknowledgement;
    request.audit_policy.validate().map_err(|_| invalid)?;
    request
        .approval
        .verify(&request.audit_policy, response.authoritative_height)
        .map_err(|_| invalid)?;
    let approval_auditor = request
        .audit_policy
        .body
        .auditors
        .iter()
        .find(|auditor| auditor.auditor_id == request.approval.body.auditor_id)
        .ok_or(invalid)?;
    let lifecycle_is_exact = if response.collected < response.required {
        response.lifecycle == PrivateSettlementLifecycleDtoV1::Collecting
    } else {
        response.lifecycle == PrivateSettlementLifecycleDtoV1::Audited
    };
    if response.authoritative_height == 0
        || response.authoritative_height > request.approval.body.expiry_height
        || response.payload_digest != requested_payload_digest
        || response.bundle_id != request.approval.body.bundle_id
        || response.leg_ordinal != request.approval.body.leg_ordinal
        || response.committee_authority.route.dataspace_id != request.approval.body.dataspace_id
        || request.audit_policy.body.dataspace_id != request.approval.body.dataspace_id
        || request.audit_policy.policy_digest != request.approval.body.audit_policy_digest
        || request.audit_policy.body.key_epoch != request.approval.body.audit_key_epoch
        || response.collected == 0
        || response.required == 0
        || response.required != request.audit_policy.body.min_approvals
        || response.collected > response.required
        || usize::from(response.collected) > request.audit_policy.body.auditors.len()
        || !lifecycle_is_exact
    {
        return Err(invalid);
    }
    response
        .committee_authority
        .validate()
        .map_err(|_| invalid)?;
    for (validator, pop) in response
        .committee_authority
        .validators
        .iter()
        .zip(&response.committee_authority.validator_pops)
    {
        if validator.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
            || iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err()
            || validator.public_key() == &approval_auditor.signing_key
        {
            return Err(invalid);
        }
    }
    let approval_digest = request.approval.digest().map_err(|_| invalid)?;
    let acknowledgement_digest = response.acknowledgement_digest().map_err(|_| invalid)?;
    let authority_digest = response.committee_authority.digest().map_err(|_| invalid)?;
    let attestation = &response.responder_attestation;
    attestation.validate_shape().map_err(|_| invalid)?;
    let expected_body =
        iroha_data_model::nexus::PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: request.approval.body.network_id,
            payload_digest: requested_payload_digest,
            approval_digest,
            acknowledgement_digest,
            authority_digest,
            lifecycle_code: response.lifecycle.attestation_code(),
            authoritative_height: response.authoritative_height,
            responder: attestation.body.responder.clone(),
        };
    if attestation.body != expected_body {
        return Err(invalid);
    }
    let responder_index = response
        .committee_authority
        .validators
        .iter()
        .position(|validator| validator == &attestation.body.responder)
        .ok_or(invalid)?;
    let signature = Signature::try_from_bytes(&attestation.signature).map_err(|_| invalid)?;
    signature
        .verify(
            attestation.body.responder.public_key(),
            &attestation.body.signature_preimage().map_err(|_| invalid)?,
        )
        .map_err(|_| invalid)?;
    Ok(responder_index)
}

/// Sponsor-authenticated Prepare-lock registration, finalization, or abort
/// carrier submission.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementBundleSubmitRequestV1 {
    /// Exact sponsor-signed transaction carrying one Prepare-lock registration,
    /// finalization, or abort instruction.
    pub transaction: SignedTransaction,
}

/// Redacted admission response for one globally submitted carrier.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementBundleSubmitResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Height observed when the carrier entered transaction admission.
    pub accepted_at_height: u64,
    /// Public carrier hash or transaction identifier assigned by admission.
    pub carrier_id: Hash,
}

/// Public allowlisted bundle status.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementBundleStatusResponseV1 {
    /// Exact public bundle manifest when locally retained.
    ///
    /// An opaque global abort marker can outlive restricted sidecars, so
    /// terminal abort status remains queryable without manufacturing fields.
    #[norito(required)]
    pub manifest: Option<AtomicPrivateSettlementV1>,
    /// Current public lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
    /// Finalized height when terminal success is durable.
    #[norito(required)]
    pub finalized_height: Option<u64>,
}

/// Public receipt query result.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(tag = "status", content = "value", rename_all = "snake_case")]
pub enum PrivateSettlementBundleReceiptResponseV1 {
    /// The bundle is known but has no public terminal receipt yet.
    Pending {
        /// Public bundle identifier.
        bundle_id: Hash,
        /// Current public lifecycle.
        lifecycle: PrivateSettlementLifecycleDtoV1,
    },
    /// Every private delta finalized atomically.
    Finalized(PrivateSettlementReceiptV1),
    /// The bundle terminated without applying any private delta.
    Aborted(PrivateSettlementAbortReceiptV1),
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{HashOf, HybridKeyPair, KeyPair, SignatureOf};
    use iroha_data_model::{
        account::AccountId,
        block::BlockHeader,
        nexus::{
            DataSpaceId, LaneId, PRIVATE_SETTLEMENT_ML_KEM_768_CIPHERTEXT_BYTES_V1,
            PRIVATE_SETTLEMENT_WRAPPED_DEK_BYTES_V1, PrivateSettlementAuditAadV1,
            PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1,
            PrivateSettlementAuditApprovalBodyV1, PrivateSettlementAuditPolicyBodyV1,
            PrivateSettlementAuditorV1, PrivateSettlementAuditorViewAttestationBodyV1,
            PrivateSettlementCapsulePaddingV1, PrivateSettlementHybridPublicKeyV1,
            PrivateSettlementLegCommitmentV1, PrivateSettlementProofProfileV1,
            PrivateSettlementWrappedDekV1,
        },
        peer::PeerId,
        privacy::{
            PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1, PrivacyCommitmentV1,
            PrivacyEncryptedOutputV1, PrivacyEncryptionKeyV1, PrivacyNullifierV1, PrivacyPoolIdV1,
            PrivacyRecipientIdV1, PrivacyRootV1,
        },
        transaction::FeePaymentIntent,
    };

    struct ResponseValidationFixtureV1 {
        network_id: NetworkId,
        payload_digest: Hash,
        auditor_signing: KeyPair,
        validator_keys: Vec<KeyPair>,
        committee: PrivateSettlementCommitteeProofResponseV1,
        capsule_request: PrivateSettlementAuditorCapsuleRequestV1,
        auditor: PrivateSettlementAuditorCapsuleResponseV1,
        approval_request: PrivateSettlementAuditApprovalRequestV1,
        approval_response: PrivateSettlementAuditApprovalResponseV1,
    }

    fn validation_network(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new([
            seed,
        ])))
    }

    fn validation_route(dataspace: u64) -> PrivateSettlementRouteV1 {
        PrivateSettlementRouteV1 {
            dataspace_id: DataSpaceId::new(dataspace),
            lane_id: LaneId::new(u32::try_from(dataspace).expect("fixture dataspace fits lane")),
            lane_incarnation: Hash::new(dataspace.to_le_bytes()),
        }
    }

    fn validation_output(seed: u8) -> PrivacyEncryptedOutputV1 {
        let commitment = PrivacyCommitmentV1::new([seed; 32]);
        let mut ciphertext = vec![seed; PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1];
        ciphertext[..4].copy_from_slice(b"IPNE");
        PrivacyEncryptedOutputV1 {
            recipient: PrivacyRecipientIdV1::new([seed.wrapping_add(0x10); 32]),
            ephemeral_public_key: PrivacyEncryptionKeyV1::new([seed.wrapping_add(0x20); 32]),
            commitment,
            ciphertext,
        }
    }

    fn sign_bytes(key: &KeyPair, preimage: &[u8]) -> Vec<u8> {
        Signature::try_new(key.private_key(), preimage)
            .expect("fixture signature")
            .payload()
            .to_vec()
    }

    fn attest_auditor_response_v1(
        response: &mut PrivateSettlementAuditorCapsuleResponseV1,
        payload_digest: Hash,
        responder_key: &KeyPair,
    ) {
        let body = PrivateSettlementAuditorViewAttestationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: response.manifest.network_id,
            payload_digest,
            view_digest: response.view_digest().expect("fixture auditor view digest"),
            authority_digest: response
                .committee_authority
                .digest()
                .expect("fixture authority digest"),
            lifecycle_code: response.lifecycle.attestation_code(),
            authoritative_height: response.authoritative_height,
            responder: response.committee_authority.validators[0].clone(),
        };
        response.responder_attestation = PrivateSettlementAuditorViewAttestationV1 {
            signature: sign_bytes(
                responder_key,
                &body
                    .signature_preimage()
                    .expect("fixture auditor attestation preimage"),
            ),
            body,
        };
    }

    fn successor_policy_v1(
        historical_policy: &PrivateSettlementAuditPolicyV1,
    ) -> (PrivateSettlementAuditPolicyV1, KeyPair) {
        let signing = KeyPair::from_seed(vec![0x42; 32], Algorithm::Ed25519);
        let mut hybrid_rng =
            iroha_crypto::rng_from_seed_slice(b"shared response verifier successor auditor");
        let hybrid = HybridKeyPair::generate(&mut hybrid_rng).expect("successor hybrid key");
        let stable_auditor_id = historical_policy.body.auditors[0].auditor_id.clone();
        let policy = PrivateSettlementAuditPolicyV1::new(PrivateSettlementAuditPolicyBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            dataspace_id: historical_policy.body.dataspace_id,
            policy_id: historical_policy.body.policy_id,
            revision: historical_policy.body.revision + 1,
            key_epoch: historical_policy.body.key_epoch + 1,
            activation_height: 15,
            retirement_height: Some(500),
            min_approvals: 1,
            auditors: vec![PrivateSettlementAuditorV1 {
                auditor_id: stable_auditor_id,
                signing_key: signing.public_key().clone(),
                encryption_key: PrivateSettlementHybridPublicKeyV1::from_hybrid(hybrid.public()),
            }],
        })
        .expect("fixture successor policy");
        (policy, signing)
    }

    fn response_validation_fixture_v1() -> ResponseValidationFixtureV1 {
        let network_id = validation_network(0x31);
        let route = validation_route(7);
        let second_route = validation_route(8);
        let auditor_signing = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
        let auditor_id = AccountId::new(auditor_signing.public_key().clone());
        let mut hybrid_rng = iroha_crypto::rng_from_seed_slice(b"shared response verifier auditor");
        let hybrid = HybridKeyPair::generate(&mut hybrid_rng).expect("fixture hybrid key");
        let audit_policy =
            PrivateSettlementAuditPolicyV1::new(PrivateSettlementAuditPolicyBodyV1 {
                version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
                dataspace_id: route.dataspace_id,
                policy_id: Hash::new(b"shared response verifier policy"),
                revision: 1,
                key_epoch: 1,
                activation_height: 5,
                retirement_height: Some(20),
                min_approvals: 1,
                auditors: vec![PrivateSettlementAuditorV1 {
                    auditor_id: auditor_id.clone(),
                    signing_key: auditor_signing.public_key().clone(),
                    encryption_key: PrivateSettlementHybridPublicKeyV1::from_hybrid(
                        hybrid.public(),
                    ),
                }],
            })
            .expect("fixture audit policy");

        let mut validator_keys = (0_u8..4)
            .map(|index| KeyPair::from_seed(vec![0x50 + index; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        validator_keys.sort_by(|left, right| {
            PeerId::from(left.public_key().clone()).cmp(&PeerId::from(right.public_key().clone()))
        });
        let validators = validator_keys
            .iter()
            .map(|key| PeerId::from(key.public_key().clone()))
            .collect::<Vec<_>>();
        let authority = PrivateSettlementCommitteeAuthorityV1 {
            route,
            validator_set_hash: HashOf::new(&validators),
            validators,
            validator_pops: validator_keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("fixture validator proof of possession")
                })
                .collect(),
        };
        authority.validate().expect("fixture authority");
        let authority_digest = authority.digest().expect("fixture authority digest");

        let sponsor = KeyPair::from_seed(vec![0x61; 32], Algorithm::Ed25519);
        let mut manifest = AtomicPrivateSettlementV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id,
            bundle_id: Hash::new(b"shared response verifier provisional bundle"),
            authority_context_height: 10,
            expiry_height: 100,
            sponsor: AccountId::new(sponsor.public_key().clone()),
            public_fee_intent: FeePaymentIntent::authority(Vec::new(), None),
            fee_intent_digest: Hash::new(b"shared response verifier provisional fee"),
            reimbursement_terms_commitment: Hash::new(
                b"shared response verifier reimbursement terms",
            ),
            reimbursement_leg_ordinal: 0,
            legs: vec![
                PrivateSettlementLegCommitmentV1 {
                    ordinal: 0,
                    route,
                    pool_id: PrivacyPoolIdV1::new([0x71; 32]),
                    asset_binding_commitment: Hash::new(b"shared response verifier asset"),
                    audit_policy_digest: audit_policy.policy_digest,
                    payload_digest: Hash::new(b"shared response verifier provisional payload"),
                    availability_certificate_digest: Hash::new(
                        b"shared response verifier provisional availability",
                    ),
                    delta_digest: Hash::new(b"shared response verifier provisional delta"),
                },
                PrivateSettlementLegCommitmentV1 {
                    ordinal: 1,
                    route: second_route,
                    pool_id: PrivacyPoolIdV1::new([0x72; 32]),
                    asset_binding_commitment: Hash::new(b"shared response verifier second asset"),
                    audit_policy_digest: Hash::new(b"shared response verifier second policy"),
                    payload_digest: Hash::new(b"shared response verifier second payload"),
                    availability_certificate_digest: Hash::new(
                        b"shared response verifier second availability",
                    ),
                    delta_digest: Hash::new(b"shared response verifier second delta"),
                },
            ],
        };
        manifest.fee_intent_digest = manifest
            .computed_fee_intent_digest()
            .expect("fixture fee intent digest");
        manifest.bundle_id = manifest.computed_bundle_id().expect("fixture bundle id");

        let padding = PrivateSettlementCapsulePaddingV1::KiB4;
        let audit_plaintext_commitment = Hash::new(b"shared response verifier audit plaintext");
        let audit_capsule = PrivateSettlementAuditCapsuleV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            aad: PrivateSettlementAuditAadV1 {
                network_id,
                bundle_id: manifest.bundle_id,
                leg_ordinal: 0,
                route,
                authority_digest,
                authority_context_height: manifest.authority_context_height,
                audit_policy_digest: audit_policy.policy_digest,
                audit_key_epoch: audit_policy.body.key_epoch,
                plaintext_commitment: audit_plaintext_commitment,
            },
            padding,
            nonce: [0x81; 24],
            ciphertext: vec![0x82; padding.ciphertext_bytes()],
            wrapped_deks: vec![PrivateSettlementWrappedDekV1 {
                auditor_id: auditor_id.clone(),
                ephemeral_x25519: [0x83; 32],
                ml_kem_ciphertext: vec![0x84; PRIVATE_SETTLEMENT_ML_KEM_768_CIPHERTEXT_BYTES_V1],
                nonce: [0x85; 24],
                wrapped_dek: vec![0x86; PRIVATE_SETTLEMENT_WRAPPED_DEK_BYTES_V1],
            }],
        };
        audit_capsule
            .validate_against(&audit_policy)
            .expect("fixture audit capsule");
        let capsule_digest = audit_capsule.digest().expect("fixture capsule digest");
        let encrypted_outputs = vec![
            validation_output(0x91),
            validation_output(0x92),
            validation_output(0x93),
        ];
        let profile = PrivateSettlementProofProfileV1::IvmPrivateNoteFixed2In3Out;
        let statement = PrivateSettlementProofStatementV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            profile,
            proof_profile_digest: profile.digest(),
            network_id,
            bundle_id: manifest.bundle_id,
            leg_ordinal: 0,
            route,
            authority_context_height: manifest.authority_context_height,
            pool_id: manifest.legs[0].pool_id,
            asset_binding_commitment: manifest.legs[0].asset_binding_commitment,
            old_root: PrivacyRootV1::new([0xA1; 32]),
            new_root: PrivacyRootV1::new([0xA4; 32]),
            old_epoch: 1,
            new_epoch: 2,
            nullifiers: vec![
                PrivacyNullifierV1::new([0xA2; 32]),
                PrivacyNullifierV1::new([0xA3; 32]),
            ],
            output_commitments: encrypted_outputs
                .iter()
                .map(|output| output.commitment)
                .collect(),
            encrypted_outputs: encrypted_outputs.clone(),
            audit_plaintext_commitment,
            audit_capsule_digest: capsule_digest,
            audit_policy_digest: audit_policy.policy_digest,
            audit_key_epoch: audit_policy.body.key_epoch,
            fee_intent_digest: manifest.fee_intent_digest,
            reimbursement_terms_commitment: manifest.reimbursement_terms_commitment,
            reimbursement_leg_ordinal: manifest.reimbursement_leg_ordinal,
            expiry_height: manifest.expiry_height,
        };
        statement.validate().expect("fixture proof statement");
        let proof = vec![0xB1; 128];
        let proof_digest = private_settlement_proof_digest_v1(&proof);
        let delta = PrivateSettlementDeltaV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            bundle_id: statement.bundle_id,
            leg_ordinal: statement.leg_ordinal,
            route,
            pool_id: statement.pool_id,
            asset_binding_commitment: statement.asset_binding_commitment,
            old_root: statement.old_root,
            new_root: statement.new_root,
            old_epoch: statement.old_epoch,
            new_epoch: statement.new_epoch,
            nullifiers: statement.nullifiers.clone(),
            output_commitments: statement.output_commitments.clone(),
            encrypted_outputs,
            statement_digest: statement.digest().expect("fixture statement digest"),
            proof_digest,
            capsule_digest,
            audit_policy_digest: audit_policy.policy_digest,
            audit_key_epoch: audit_policy.body.key_epoch,
        };
        delta
            .validate_against(&statement)
            .expect("fixture private delta");
        let delta_digest = delta.digest().expect("fixture delta digest");
        let payload_digest = Hash::new(b"shared response verifier exact payload");
        let availability_body =
            iroha_data_model::nexus::PrivateSettlementSidecarAvailabilityBodyV1 {
                version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
                network_id,
                bundle_id: manifest.bundle_id,
                leg_ordinal: 0,
                route,
                authority_digest,
                authority_context_height: manifest.authority_context_height,
                payload_digest,
                payload_bytes: 4096,
                retention_until_height: 120,
            };
        let availability_preimage = availability_body
            .signature_preimage()
            .expect("fixture availability preimage");
        let availability_signatures = validator_keys[..3]
            .iter()
            .map(|key| sign_bytes(key, &availability_preimage))
            .collect::<Vec<_>>();
        let availability_signature_refs = availability_signatures
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let availability = PrivateSettlementSidecarAvailabilityV1 {
            body: availability_body,
            signers_bitmap: 0b0111,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &availability_signature_refs,
            )
            .expect("fixture availability aggregate"),
        };
        manifest.legs[0].payload_digest = payload_digest;
        manifest.legs[0].delta_digest = delta_digest;
        manifest.legs[0].availability_certificate_digest =
            availability.digest().expect("fixture availability digest");
        manifest.validate().expect("fixture final manifest");

        let approval_body = PrivateSettlementAuditApprovalBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id,
            bundle_id: manifest.bundle_id,
            leg_ordinal: 0,
            dataspace_id: route.dataspace_id,
            auditor_id,
            audit_policy_digest: audit_policy.policy_digest,
            audit_key_epoch: audit_policy.body.key_epoch,
            proof_digest,
            capsule_digest,
            delta_digest,
            old_root: delta.old_root,
            new_root: delta.new_root,
            expiry_height: manifest.expiry_height,
        };
        let approval = PrivateSettlementAuditApprovalV1 {
            signature: SignatureOf::try_new(auditor_signing.private_key(), &approval_body)
                .expect("fixture auditor approval"),
            body: approval_body,
        };
        approval
            .verify(&audit_policy, manifest.authority_context_height)
            .expect("fixture governed approval");
        let approval_request = PrivateSettlementAuditApprovalRequestV1 {
            audit_policy: audit_policy.clone(),
            approval: approval.clone(),
        };
        let capsule_request = PrivateSettlementAuditorCapsuleRequestV1 {
            audit_policy: audit_policy.clone(),
        };

        let committee = PrivateSettlementCommitteeProofResponseV1 {
            manifest: manifest.clone(),
            audit_policy: audit_policy.clone(),
            committee_authority: authority.clone(),
            statement: statement.clone(),
            proof,
            delta: delta.clone(),
            audit_approvals: vec![approval],
            audit_capsule_digest: capsule_digest,
            availability: availability.clone(),
            lifecycle: PrivateSettlementLifecycleDtoV1::Audited,
        };

        let placeholder_view_body = PrivateSettlementAuditorViewAttestationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id,
            payload_digest,
            view_digest: Hash::new(b"shared response verifier placeholder view"),
            authority_digest,
            lifecycle_code: PrivateSettlementLifecycleDtoV1::Collecting.attestation_code(),
            authoritative_height: 11,
            responder: authority.validators[0].clone(),
        };
        let mut auditor = PrivateSettlementAuditorCapsuleResponseV1 {
            authoritative_height: 11,
            manifest: manifest.clone(),
            audit_policy: audit_policy.clone(),
            access_audit_policy: audit_policy.clone(),
            committee_authority: authority.clone(),
            statement,
            delta,
            audit_capsule,
            availability,
            lifecycle: PrivateSettlementLifecycleDtoV1::Collecting,
            responder_attestation: PrivateSettlementAuditorViewAttestationV1 {
                body: placeholder_view_body,
                signature: vec![0; 96],
            },
        };
        let view_body = PrivateSettlementAuditorViewAttestationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id,
            payload_digest,
            view_digest: auditor.view_digest().expect("fixture auditor view digest"),
            authority_digest,
            lifecycle_code: auditor.lifecycle.attestation_code(),
            authoritative_height: auditor.authoritative_height,
            responder: authority.validators[0].clone(),
        };
        auditor.responder_attestation = PrivateSettlementAuditorViewAttestationV1 {
            signature: sign_bytes(
                &validator_keys[0],
                &view_body
                    .signature_preimage()
                    .expect("fixture auditor attestation preimage"),
            ),
            body: view_body,
        };

        let placeholder_ack_body = PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id,
            payload_digest,
            approval_digest: Hash::new(b"shared response verifier placeholder approval"),
            acknowledgement_digest: Hash::new(
                b"shared response verifier placeholder acknowledgement",
            ),
            authority_digest,
            lifecycle_code: PrivateSettlementLifecycleDtoV1::Audited.attestation_code(),
            authoritative_height: 12,
            responder: authority.validators[0].clone(),
        };
        let mut approval_response = PrivateSettlementAuditApprovalResponseV1 {
            authoritative_height: 12,
            bundle_id: manifest.bundle_id,
            payload_digest,
            leg_ordinal: 0,
            committee_authority: authority,
            collected: 1,
            required: 1,
            newly_recorded: true,
            lifecycle: PrivateSettlementLifecycleDtoV1::Audited,
            responder_attestation: PrivateSettlementAuditApprovalAcknowledgementAttestationV1 {
                body: placeholder_ack_body,
                signature: vec![0; 96],
            },
        };
        let acknowledgement_body = PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id,
            payload_digest,
            approval_digest: approval_request
                .approval
                .digest()
                .expect("fixture approval digest"),
            acknowledgement_digest: approval_response
                .acknowledgement_digest()
                .expect("fixture acknowledgement digest"),
            authority_digest,
            lifecycle_code: approval_response.lifecycle.attestation_code(),
            authoritative_height: approval_response.authoritative_height,
            responder: approval_response.committee_authority.validators[0].clone(),
        };
        approval_response.responder_attestation =
            PrivateSettlementAuditApprovalAcknowledgementAttestationV1 {
                signature: sign_bytes(
                    &validator_keys[0],
                    &acknowledgement_body
                        .signature_preimage()
                        .expect("fixture acknowledgement attestation preimage"),
                ),
                body: acknowledgement_body,
            };

        ResponseValidationFixtureV1 {
            network_id,
            payload_digest,
            auditor_signing,
            validator_keys,
            committee,
            capsule_request,
            auditor,
            approval_request,
            approval_response,
        }
    }

    #[test]
    fn shared_committee_response_verifier_binds_network_payload_and_private_material() {
        let fixture = response_validation_fixture_v1();
        validate_private_settlement_committee_proof_response_v1(
            &fixture.network_id,
            fixture.payload_digest,
            &fixture.committee,
        )
        .expect("exact committee response verifies");

        assert_eq!(
            validate_private_settlement_committee_proof_response_v1(
                &validation_network(0x32),
                fixture.payload_digest,
                &fixture.committee,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidCommitteeProofResponse)
        );
        let mut substituted = fixture.committee;
        substituted.delta.proof_digest = Hash::new(b"substituted committee proof digest");
        assert_eq!(
            validate_private_settlement_committee_proof_response_v1(
                &fixture.network_id,
                fixture.payload_digest,
                &substituted,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidCommitteeProofResponse)
        );

        let mut substituted_policy = substituted;
        substituted_policy.delta.proof_digest =
            private_settlement_proof_digest_v1(&substituted_policy.proof);
        substituted_policy.statement.audit_policy_digest =
            Hash::new(b"substituted statement policy");
        substituted_policy.statement.audit_key_epoch += 1;
        substituted_policy.delta.audit_policy_digest =
            substituted_policy.statement.audit_policy_digest;
        substituted_policy.delta.audit_key_epoch = substituted_policy.statement.audit_key_epoch;
        substituted_policy.delta.statement_digest = substituted_policy
            .statement
            .digest()
            .expect("substituted statement remains encodable");
        let substituted_delta_digest = substituted_policy
            .delta
            .digest()
            .expect("substituted delta remains encodable");
        substituted_policy.manifest.legs[0].delta_digest = substituted_delta_digest;
        substituted_policy.audit_approvals[0].body.delta_digest = substituted_delta_digest;
        let substituted_approval_signature = SignatureOf::try_new(
            fixture.auditor_signing.private_key(),
            &substituted_policy.audit_approvals[0].body,
        )
        .expect("substituted approval remains internally signed");
        substituted_policy.audit_approvals[0].signature = substituted_approval_signature;
        assert_eq!(
            validate_private_settlement_committee_proof_response_v1(
                &fixture.network_id,
                fixture.payload_digest,
                &substituted_policy,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidCommitteeProofResponse)
        );
    }

    #[test]
    fn shared_auditor_response_verifier_authenticates_exact_responder_and_network() {
        let fixture = response_validation_fixture_v1();
        assert_eq!(
            validate_private_settlement_auditor_capsule_response_v1(
                &fixture.network_id,
                fixture.payload_digest,
                &fixture.capsule_request,
                &fixture.auditor,
            ),
            Ok(0)
        );
        assert_eq!(
            validate_private_settlement_auditor_identity_v1(
                fixture.auditor_signing.public_key(),
                &fixture.auditor,
            ),
            Ok(())
        );

        assert_eq!(
            validate_private_settlement_auditor_capsule_response_v1(
                &validation_network(0x33),
                fixture.payload_digest,
                &fixture.capsule_request,
                &fixture.auditor,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditorCapsuleResponse)
        );
        let mut substituted = fixture.auditor;
        substituted.responder_attestation.signature = sign_bytes(
            &fixture.validator_keys[1],
            &substituted
                .responder_attestation
                .body
                .signature_preimage()
                .expect("substituted response remains encodable"),
        );
        assert_eq!(
            validate_private_settlement_auditor_capsule_response_v1(
                &fixture.network_id,
                fixture.payload_digest,
                &fixture.capsule_request,
                &substituted,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditorCapsuleResponse)
        );
    }

    #[test]
    fn shared_auditor_response_binds_requested_policy_and_retained_successor_access() {
        let fixture = response_validation_fixture_v1();
        let (successor_policy, successor_signing) =
            successor_policy_v1(&fixture.auditor.audit_policy);
        let successor_request = PrivateSettlementAuditorCapsuleRequestV1 {
            audit_policy: successor_policy.clone(),
        };
        let original_view_digest = fixture
            .auditor
            .view_digest()
            .expect("fixture original view digest");
        let mut access_substituted = fixture.auditor.clone();
        access_substituted.access_audit_policy = successor_policy.clone();
        assert_ne!(
            access_substituted
                .view_digest()
                .expect("fixture substituted view digest"),
            original_view_digest,
            "the responder view digest includes the complete access policy"
        );
        let mut overlapping = fixture.auditor.clone();
        overlapping.access_audit_policy = successor_policy.clone();
        overlapping.authoritative_height = 19;
        attest_auditor_response_v1(
            &mut overlapping,
            fixture.payload_digest,
            &fixture.validator_keys[0],
        );
        assert_eq!(
            validate_private_settlement_auditor_capsule_response_v1(
                &fixture.network_id,
                fixture.payload_digest,
                &successor_request,
                &overlapping,
            ),
            Ok(0),
            "the signed view does not second-guess an overlapping WSV governance rotation"
        );

        let mut retained = fixture.auditor.clone();
        retained.access_audit_policy = successor_policy;
        retained.authoritative_height = 110;
        retained.lifecycle = PrivateSettlementLifecycleDtoV1::Finalized;

        assert_eq!(
            validate_private_settlement_auditor_capsule_response_v1(
                &fixture.network_id,
                fixture.payload_digest,
                &successor_request,
                &retained,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditorCapsuleResponse),
            "the responder signature must bind the access policy"
        );

        attest_auditor_response_v1(
            &mut retained,
            fixture.payload_digest,
            &fixture.validator_keys[0],
        );
        assert_eq!(
            validate_private_settlement_auditor_capsule_response_v1(
                &fixture.network_id,
                fixture.payload_digest,
                &successor_request,
                &retained,
            ),
            Ok(0),
            "an overlapping, preactivated successor may read retained terminal material"
        );
        assert_eq!(
            validate_private_settlement_auditor_identity_v1(
                successor_signing.public_key(),
                &retained,
            ),
            Ok(()),
            "successor signing keys map through the stable historical auditor identity"
        );
        assert_eq!(
            validate_private_settlement_auditor_capsule_response_v1(
                &fixture.network_id,
                fixture.payload_digest,
                &fixture.capsule_request,
                &retained,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditorCapsuleResponse),
            "the response access policy must equal the exact request policy"
        );

        let mut live_after_expiry = retained.clone();
        live_after_expiry.lifecycle = PrivateSettlementLifecycleDtoV1::Collecting;
        attest_auditor_response_v1(
            &mut live_after_expiry,
            fixture.payload_digest,
            &fixture.validator_keys[0],
        );
        assert_eq!(
            validate_private_settlement_auditor_capsule_response_v1(
                &fixture.network_id,
                fixture.payload_digest,
                &successor_request,
                &live_after_expiry,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditorCapsuleResponse),
            "non-terminal reads remain bounded by bundle expiry"
        );

        let mut outside_retention = retained;
        outside_retention.authoritative_height = 121;
        attest_auditor_response_v1(
            &mut outside_retention,
            fixture.payload_digest,
            &fixture.validator_keys[0],
        );
        assert_eq!(
            validate_private_settlement_auditor_capsule_response_v1(
                &fixture.network_id,
                fixture.payload_digest,
                &successor_request,
                &outside_retention,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditorCapsuleResponse),
            "terminal reads fail closed after restricted-DA retention"
        );
    }

    #[test]
    fn shared_auditor_identity_verifier_rejects_ungoverned_and_consensus_keys() {
        let fixture = response_validation_fixture_v1();
        let unknown = KeyPair::from_seed(vec![0xC1; 32], Algorithm::Ed25519);
        assert_eq!(
            validate_private_settlement_auditor_identity_v1(unknown.public_key(), &fixture.auditor,),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditorKeySeparation)
        );

        let mut reused = fixture.auditor;
        let consensus_key = reused.committee_authority.validators[0]
            .public_key()
            .clone();
        let mut reused_body = reused.audit_policy.body.clone();
        reused_body.auditors[0].signing_key = consensus_key.clone();
        let reused_policy =
            PrivateSettlementAuditPolicyV1::new(reused_body).expect("reused policy is well formed");
        reused.audit_policy = reused_policy.clone();
        reused.access_audit_policy = reused_policy;
        assert_eq!(
            validate_private_settlement_auditor_identity_v1(&consensus_key, &reused),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditorKeySeparation)
        );
    }

    #[test]
    fn shared_approval_response_verifier_binds_exact_request_height_and_responder() {
        let fixture = response_validation_fixture_v1();
        assert_eq!(
            validate_private_settlement_audit_approval_response_v1(
                fixture.payload_digest,
                &fixture.approval_request,
                &fixture.approval_response,
            ),
            Ok(0)
        );

        let mut substituted_request = fixture.approval_request.clone();
        substituted_request.approval.body.expiry_height -= 1;
        assert_eq!(
            validate_private_settlement_audit_approval_response_v1(
                fixture.payload_digest,
                &substituted_request,
                &fixture.approval_response,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditApprovalAcknowledgement)
        );

        let mut wrong_policy = fixture.approval_request.clone();
        let mut wrong_policy_body = wrong_policy.audit_policy.body.clone();
        wrong_policy_body.revision += 1;
        wrong_policy_body.key_epoch += 1;
        wrong_policy.audit_policy = PrivateSettlementAuditPolicyV1::new(wrong_policy_body)
            .expect("substituted active policy remains well formed");
        assert_eq!(
            validate_private_settlement_audit_approval_response_v1(
                fixture.payload_digest,
                &wrong_policy,
                &fixture.approval_response,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditApprovalAcknowledgement),
            "the signed approval must match the exact submitted policy"
        );
        let mut expired = fixture.approval_response;
        expired.authoritative_height = substituted_request.approval.body.expiry_height + 1;
        assert_eq!(
            validate_private_settlement_audit_approval_response_v1(
                fixture.payload_digest,
                &substituted_request,
                &expired,
            ),
            Err(PrivateSettlementResponseValidationErrorV1::InvalidAuditApprovalAcknowledgement)
        );
    }

    #[test]
    fn shared_response_validation_errors_are_redacted() {
        let sensitive_canary = "account=alice amount=424242 memo=classified";
        for error in [
            PrivateSettlementResponseValidationErrorV1::InvalidCommitteeProofResponse,
            PrivateSettlementResponseValidationErrorV1::InvalidAuditorCapsuleResponse,
            PrivateSettlementResponseValidationErrorV1::InvalidAuditorKeySeparation,
            PrivateSettlementResponseValidationErrorV1::InvalidAuditApprovalAcknowledgement,
        ] {
            let rendered = format!("{error}: {sensitive_canary}");
            assert!(!error.to_string().contains(sensitive_canary));
            assert!(rendered.starts_with(&error.to_string()));
        }
    }

    #[test]
    fn auditor_capsule_json_requires_authoritative_height() {
        let error = norito::json::from_json::<PrivateSettlementAuditorCapsuleResponseV1>("{}")
            .expect_err("authoritative response height must be explicit");
        assert!(error.to_string().contains("authoritative_height"));
    }

    #[test]
    fn auditor_capsule_request_json_requires_exact_policy() {
        let error = norito::json::from_json::<PrivateSettlementAuditorCapsuleRequestV1>("{}")
            .expect_err("the current access policy must be explicit");
        assert!(error.to_string().contains("audit_policy"));
    }

    #[test]
    fn audit_approval_acknowledgement_json_requires_authoritative_height() {
        let error = norito::json::from_json::<PrivateSettlementAuditApprovalResponseV1>("{}")
            .expect_err("attested acknowledgement height must be explicit");
        assert!(error.to_string().contains("authoritative_height"));
    }

    #[test]
    fn bundle_status_json_requires_explicit_manifest() {
        let response = PrivateSettlementBundleStatusResponseV1 {
            manifest: None,
            lifecycle: PrivateSettlementLifecycleDtoV1::Aborted,
            finalized_height: None,
        };
        let value = norito::json::to_value(&response).expect("encode bundle status JSON");
        assert_eq!(value.get("manifest"), Some(&norito::json::Value::Null));
        let decoded =
            norito::json::from_value::<PrivateSettlementBundleStatusResponseV1>(value.clone())
                .expect("explicit null manifest decodes");
        assert_eq!(decoded, response);

        let mut omitted = value;
        omitted
            .as_object_mut()
            .expect("bundle status is a JSON object")
            .remove("manifest");
        let error = norito::json::from_value::<PrivateSettlementBundleStatusResponseV1>(omitted)
            .expect_err("omitted manifest must reject");
        assert!(error.to_string().contains("missing field `manifest`"));
    }

    #[test]
    fn lifecycle_and_pending_receipt_roundtrip_canonically() {
        let response = PrivateSettlementBundleReceiptResponseV1::Pending {
            bundle_id: Hash::new(b"private-settlement-dto-roundtrip"),
            lifecycle: PrivateSettlementLifecycleDtoV1::Prepared,
        };
        let bytes = norito::encode_canonical(&response).expect("DTO encodes canonically");
        let decoded: PrivateSettlementBundleReceiptResponseV1 =
            norito::decode_canonical(&bytes).expect("DTO decodes canonically");
        assert_eq!(decoded, response);
        let json = norito::json::to_json(&response).expect("DTO JSON encodes");
        let decoded_json: PrivateSettlementBundleReceiptResponseV1 =
            norito::json::from_json(&json).expect("DTO JSON decodes");
        assert_eq!(decoded_json, response);
    }

    #[test]
    fn phase_certificate_recovery_requires_explicit_optional_fields() {
        let response = PrivateSettlementPhaseCertificatesResponseV1 {
            bundle_id: Hash::new(b"private-settlement-recovery-bundle"),
            payload_digest: Hash::new(b"private-settlement-recovery-payload"),
            leg_ordinal: 1,
            lifecycle: PrivateSettlementLifecycleDtoV1::Prepared,
            prepare_certificate: None,
            commit_certificate: None,
        };
        let value = norito::json::to_value(&response).expect("encode recovery JSON");
        assert_eq!(
            value.get("prepare_certificate"),
            Some(&norito::json::Value::Null)
        );
        assert_eq!(
            value.get("commit_certificate"),
            Some(&norito::json::Value::Null)
        );
        let decoded =
            norito::json::from_value::<PrivateSettlementPhaseCertificatesResponseV1>(value.clone())
                .expect("explicit null recovery fields decode");
        assert_eq!(decoded, response);

        let mut omitted = value;
        omitted
            .as_object_mut()
            .expect("recovery response is an object")
            .remove("prepare_certificate");
        let error =
            norito::json::from_value::<PrivateSettlementPhaseCertificatesResponseV1>(omitted)
                .expect_err("omitted recovery field must reject");
        assert!(
            error
                .to_string()
                .contains("missing field `prepare_certificate`")
        );
    }
}
